#[cfg(not(feature = "library"))]
use crate::state::{
    get_and_increment_next_subscription_id, subscriptions, SubscriptionState, NEXT_SUBSCRIPTION_ID,
};
use crate::subscription::{Cw20HookMsg, Cw721HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg};

use cosmwasm_std::{
    ensure, entry_point, from_json, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    Uint128,
};
use cw_storage_plus::Bound;

use andromeda_std::{
    ado_base::InstantiateMsg as BaseInstantiateMsg,
    ado_contract::ADOContract,
    common::{
        actions::call_action,
        context::ExecuteContext,
        denom::{
            authorize_addresses, execute_authorize_contract, execute_deauthorize_contract,
            AuthorizedAddressesResponse, PermissionAction, SEND_CW20_ACTION, SEND_NFT_ACTION,
        },
        encode_binary, OrderBy,
    },
    error::ContractError,
};

use cw20::Cw20ReceiveMsg;
use cw721::Cw721ReceiveMsg;

use cw_utils::{nonpayable, Expiration};

const MAX_LIMIT: u64 = 30;
const DEFAULT_LIMIT: u64 = 10;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-subscription";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Initialize the NEXT_SUBSCRIPTION_ID
    NEXT_SUBSCRIPTION_ID.save(deps.storage, &Uint128::from(1u128))?;

    // Set up the ADO base contract
    let inst_resp = ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info,
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;

    // Authorize specified CW721 addresses
    if let Some(authorized_token_addresses) = msg.authorized_token_addresses {
        authorize_addresses(&mut deps, SEND_NFT_ACTION, authorized_token_addresses)?;
    }

    // Authorize specified CW20 addresses
    if let Some(authorized_cw20_addresses) = msg.authorized_cw20_addresses {
        authorize_addresses(&mut deps, SEND_CW20_ACTION, authorized_cw20_addresses)?;
    }

    Ok(inst_resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);

    match msg {
        ExecuteMsg::AMPReceive(pkt) => {
            ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
        }
        _ => handle_execute(ctx, msg),
    }
}

pub fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;
    let res = match msg {
        ExecuteMsg::ReceiveNft(msg) => handle_receive_cw721(ctx, msg),
        ExecuteMsg::Receive(msg) => handle_receive_cw20(ctx, msg),
        ExecuteMsg::Cancel { nft_address } => execute_cancel(ctx, nft_address),
        ExecuteMsg::AuthorizeContract {
            action,
            addr,
            expiration,
        } => execute_authorize_contract(ctx.deps, ctx.info, action, addr, expiration),
        ExecuteMsg::DeauthorizeContract { action, addr } => {
            execute_deauthorize_contract(ctx.deps, ctx.info, action, addr)
        }
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

pub fn handle_receive_cw20(
    mut ctx: ExecuteContext,
    receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    // Validate that the CW20 token is authorized
    ADOContract::default().is_permissioned(
        ctx.deps.branch(),
        ctx.env.clone(),
        SEND_CW20_ACTION,
        ctx.info.sender.clone(),
    )?;

    let ExecuteContext {
        ref info,
        ref env,
        ref mut deps,
        ..
    } = ctx;

    // Ensure the transaction is non-payable
    nonpayable(info)?;

    let amount_sent = receive_msg.amount;
    let subscriber = receive_msg.sender.clone();

    ensure!(
        !amount_sent.is_zero(),
        ContractError::InvalidFunds {
            msg: "Cannot send a 0 amount.".to_string(),
        }
    );

    match from_json(&receive_msg.msg)? {
        Cw20HookMsg::Subscribe {
            token_id,
            nft_address,
        } => {
            // Step 1: Check for open subscription (creator address + empty subscriber)
            let open_key = (nft_address.clone(), String::new());
            let open_subscription = subscriptions()
                .may_load(deps.storage, open_key.clone())?
                .ok_or(ContractError::CustomError {
                    msg: format!(
                        "No subscription offering found for creator address {}.",
                        nft_address
                    ),
                })?;

            ensure!(
                !open_subscription.is_active,
                ContractError::CustomError {
                    msg: "This subscription is already marked as active.".to_string(),
                }
            );

            // Step 2: Check for existing subscription for this user (creator address + subscriber)
            let user_key = (nft_address.clone(), subscriber.clone());
            if let Some(existing_subscription) =
                subscriptions().may_load(deps.storage, user_key.clone())?
            {
                return Err(ContractError::CustomError {
                         msg: format!(
                            "You already have a subscription to {} offering. Please renew (if inactive) or cancel it.", 
                            existing_subscription.nft_address
                        ),
                     }
                 );
            }

            // Validate the payment amount
            ensure!(
                amount_sent == open_subscription.payment_amount,
                ContractError::InvalidFunds {
                    msg: format!(
                        "Invalid payment amount. Expected {}, received {}.",
                        open_subscription.payment_amount, amount_sent
                    ),
                }
            );

            let new_subscription = SubscriptionState {
                subscription_id: get_and_increment_next_subscription_id(deps.storage)?,
                creator: open_subscription.creator.clone(),
                subscriber: subscriber.clone(),
                token_id,
                nft_address: open_subscription.nft_address.clone(),
                start_time: Expiration::AtTime(env.block.time),
                end_time: Expiration::AtTime(
                    env.block
                        .time
                        .plus_seconds(open_subscription.subscription_duration),
                ),
                payment_amount: open_subscription.payment_amount,
                payment_pending: open_subscription.payment_amount - amount_sent, // Should Equal 0
                payment_denom: open_subscription.payment_denom.clone(),
                subscription_duration: open_subscription.subscription_duration,
                is_active: true,
            };

            subscriptions().save(deps.storage, user_key.clone(), &new_subscription)?;

            Ok(Response::new()
                .add_attribute("action", "subscribe")
                .add_attribute("subscriber", subscriber)
                .add_attribute("creator", new_subscription.creator)
                .add_attribute("creator address", new_subscription.nft_address)
                .add_attribute("start_time", new_subscription.start_time.to_string())
                .add_attribute("end_time", new_subscription.end_time.to_string())
                .add_attribute("is_active", new_subscription.is_active.to_string()))
        }
        Cw20HookMsg::Renew {
            token_id,
            nft_address,
        } => {
            let composite_key = (nft_address.clone(), subscriber.clone());
            let mut subscription = subscriptions()
                .may_load(deps.storage, composite_key.clone())?
                .ok_or(ContractError::CustomError {
                    msg: format!(
                        "No subscription found for creator address {} and subscriber {}.",
                        nft_address, subscriber
                    ),
                })?;

            // Ensure the payment amount matches
            ensure!(
                amount_sent == subscription.payment_amount,
                ContractError::InvalidFunds {
                    msg: format!(
                        "Invalid payment amount. Expected {}, received {}.",
                        subscription.payment_amount, amount_sent
                    ),
                }
            );

            if subscription.is_active {
                if let Expiration::AtTime(end_time) = subscription.end_time {
                    if env.block.time > end_time {
                        subscription.is_active = false; // Mark as inactive if expired
                        subscription.payment_pending = subscription.payment_amount;
                    } else {
                        return Err(ContractError::CustomError {
                            msg: "Subscription is already active.".to_string(),
                        });
                    }
                }
            }
            subscription.start_time = Expiration::AtTime(ctx.env.block.time);
            subscription.end_time = Expiration::AtTime(
                ctx.env
                    .block
                    .time
                    .plus_seconds(subscription.subscription_duration),
            );
            subscription.is_active = true;
            subscription.payment_pending = subscription.payment_amount - amount_sent; // Should equal 0

            // Save the updated subscription
            subscriptions().save(deps.storage, composite_key, &subscription)?;

            Ok(Response::new()
                .add_attribute("action", "renew_subscription")
                .add_attribute("subscriber", subscriber)
                .add_attribute("creator", subscription.creator)
                .add_attribute("creator address", subscription.nft_address)
                .add_attribute("token_id", token_id)
                .add_attribute("new_start_time", subscription.start_time.to_string())
                .add_attribute("new_end_time", subscription.end_time.to_string())
                .add_attribute("is_active", subscription.is_active.to_string()))
        }
    }
}

pub fn handle_receive_cw721(
    mut ctx: ExecuteContext,
    receive_msg: Cw721ReceiveMsg,
) -> Result<Response, ContractError> {
    // Validate that the NFT contract is authorized
    ADOContract::default().is_permissioned(
        ctx.deps.branch(),
        ctx.env.clone(),
        SEND_NFT_ACTION,
        ctx.info.sender.clone(),
    )?;

    let Cw721ReceiveMsg {
        sender,
        token_id,
        msg,
    } = receive_msg;
    let hook_msg: Cw721HookMsg = from_json(&msg)?;

    match hook_msg {
        Cw721HookMsg::RegisterSubscription {
            duration,
            payment_amount,
        } => {
            // Composite key: (nft_address, empty subscriber)
            let composite_key = (ctx.info.sender.to_string(), String::new());

            // Check if the subscription already exists
            if subscriptions()
                .may_load(ctx.deps.storage, composite_key.clone())?
                .is_some()
            {
                return Err(ContractError::CustomError {
                    msg: "Subscription offering already exists for this NFT.".to_string(),
                });
            }
            let subscription_id = get_and_increment_next_subscription_id(ctx.deps.storage)?;

            let subscription = SubscriptionState {
                subscription_id,
                creator: sender.clone(), // The creator is the sender of the NFT
                subscriber: String::new(), // No subscriber yet; empty string or None
                token_id,
                nft_address: ctx.info.sender.to_string(), // Address of the CW721 contract
                start_time: Expiration::Never {},         // Start time is not applicable yet
                end_time: Expiration::Never {},           // No subscription period yet
                payment_amount,
                payment_pending: payment_amount, // Full amount pending
                payment_denom: "CW20".to_string(), // Default
                subscription_duration: duration,
                is_active: false,
            };

            subscriptions().save(
                ctx.deps.storage,
                (
                    subscription.nft_address.clone(),
                    subscription.subscriber.clone(),
                ),
                &subscription,
            )?;

            Ok(Response::new()
                .add_attribute("action", "register_subscription")
                .add_attribute("creator", sender)
                .add_attribute("subscription_id", subscription_id.to_string())
                .add_attribute("token_id", subscription.token_id)
                .add_attribute("nft_address", subscription.nft_address)
                .add_attribute("duration", duration.to_string()))
        }
    }
}

pub fn execute_cancel(ctx: ExecuteContext, nft_address: String) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, env, info, ..
    } = ctx;

    let composite_key = (nft_address.clone(), info.sender.to_string());

    // Fetch the subscription
    let mut subscription = subscriptions()
        .may_load(deps.storage, composite_key.clone())?
        .ok_or(ContractError::CustomError {
            msg: format!(
                "No subscription found for address {} and subscriber {}.",
                nft_address, info.sender
            ),
        })?;

    if subscription.is_active {
        if let Expiration::AtTime(end_time) = subscription.end_time {
            if env.block.time > end_time {
                subscription.is_active = false; // Mark as inactive if expired
                subscription.payment_pending = subscription.payment_amount;
            }
        }
    }

    // Ensure the subscription is active
    if !subscription.is_active {
        return Err(ContractError::CustomError {
            msg: "Subscription is already inactive.".to_string(),
        });
    }
    subscription.is_active = false;
    subscription.payment_pending = subscription.payment_amount;
    subscription.start_time = Expiration::Never {};
    subscription.end_time = Expiration::Never {};
    subscriptions().save(deps.storage, composite_key, &subscription)?;

    Ok(Response::new()
        .add_attribute("action", "cancel_subscription")
        .add_attribute("creator", subscription.creator)
        .add_attribute("subscriber", info.sender.to_string())
        .add_attribute("is_active", subscription.is_active.to_string())
        .add_attribute("status", "cancelled"))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Subscription {
            creator,
            subscriber,
        } => encode_binary(&query_subscription(deps, creator, env, subscriber)?),
        QueryMsg::SubscriptionsForCreator {
            creator,
            start_after,
            limit,
        } => encode_binary(&query_subscriptions_for_creator(
            deps,
            creator,
            env,
            start_after,
            limit,
        )?),
        QueryMsg::SubscriptionsForSubscriber {
            subscriber,
            start_after,
            limit,
        } => encode_binary(&query_subscriptions_for_subscriber(
            deps,
            subscriber,
            env,
            start_after,
            limit,
        )?),
        QueryMsg::SubscriptionIdsForCreator {
            creator,
            start_after,
            limit,
        } => encode_binary(&query_subscription_ids_for_creator(
            deps,
            creator,
            env,
            start_after,
            limit,
        )?),
        QueryMsg::SubscriptionIdsForSubscriber {
            subscriber,
            start_after,
            limit,
        } => encode_binary(&query_subscription_ids_for_subscriber(
            deps,
            subscriber,
            env,
            start_after,
            limit,
        )?),
        QueryMsg::SubscriptionIdsForActiveSubscriptions { start_after, limit } => encode_binary(
            &query_subscription_ids_for_active_subscriptions(deps, env, start_after, limit)?,
        ),
        QueryMsg::AuthorizedAddresses {
            action,
            start_after,
            limit,
            order_by,
        } => encode_binary(&query_authorized_addresses(
            deps,
            action,
            start_after,
            limit,
            order_by,
        )?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

pub fn query_subscription(
    deps: Deps, 
    creator: String,
    env: Env,
    subscriber: String,
) -> Result<SubscriptionState, ContractError> {
    let key = (creator.clone(), subscriber.clone());
    let mut subscription =
        subscriptions()
            .may_load(deps.storage, key.clone())? 
            .ok_or(ContractError::CustomError {
                msg: format!(
                    "No subscription found for creator '{}' and subscriber '{}'.",
                    creator, subscriber
                ),
            })?;
    
    // Evaluate and potentially update the subscription's `is_active` field
    evaluate_subscription_status(&mut subscription, &env);

    Ok(subscription)
}


pub fn query_subscriptions_for_creator(
    deps: Deps,
    creator: String,
    env: Env,
    start_after: Option<(String, String)>,
    limit: Option<u64>,
) -> Result<Vec<SubscriptionState>, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    // Convert `start_after` into `Bound` if provided
    let start = start_after.map(|key| Bound::exclusive(key));

    let subscriptions = subscriptions()
        .keys(deps.storage, start, None, Order::Ascending)
        .filter_map(|res| {
            let key = res.ok()?; // Ensure key exists and is valid
            if key.0 == creator {
                Some(key)
            } else {
                None
            }
        })
        .take(limit)
        .filter_map(|key| {
            let mut subscription = subscriptions().may_load(deps.storage, key).ok().flatten()?;
            evaluate_subscription_status(&mut subscription, &env); // Evaluate `is_active`
            Some(subscription)
        })
        .collect();


    Ok(subscriptions)
}

pub fn query_subscriptions_for_subscriber(
    deps: Deps,
    subscriber: String,
    env: Env,
    start_after: Option<(String, String)>, 
    limit: Option<u64>,
) -> Result<Vec<SubscriptionState>, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(|key| Bound::exclusive(key));

    let subscriptions = subscriptions()
        .keys(deps.storage, start, None, Order::Ascending)
        .filter_map(|res| {
            let key = res.ok()?; // Ensure key exists and is valid
            if key.1 == subscriber {
                Some(key)
            } else {
                None
            }
        })
        .take(limit)
        .filter_map(|key| {
            let mut subscription = subscriptions().may_load(deps.storage, key).ok().flatten()?;
            evaluate_subscription_status(&mut subscription, &env); // Evaluate `is_active`
            Some(subscription)
        })
        .collect();

    Ok(subscriptions)
}

pub fn query_subscription_ids_for_creator(
    deps: Deps,
    creator: String,
    env: Env,
    start_after: Option<(String, String)>,
    limit: Option<u64>,
) -> Result<Vec<Uint128>, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(|key| Bound::exclusive(key));

    let subscription_ids = subscriptions()
        .keys(deps.storage, start, None, Order::Ascending)
        .filter_map(|res| {
            let key = res.ok()?;
            if key.0 == creator {
                Some(key)
            } else {
                None
            }
        })
        .take(limit)
        .filter_map(|key| {
            let mut subscription = subscriptions().may_load(deps.storage, key).ok().flatten()?;
            evaluate_subscription_status(&mut subscription, &env); // Evaluate `is_active`
            Some(subscription.subscription_id)
        })
        .collect();

    Ok(subscription_ids)
}

pub fn query_subscription_ids_for_subscriber(
    deps: Deps,
    subscriber: String,
    env: Env,
    start_after: Option<(String, String)>,
    limit: Option<u64>,
) -> Result<Vec<Uint128>, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(|key| Bound::exclusive(key));

    let subscription_ids = subscriptions()
        .keys(deps.storage, start, None, Order::Ascending)
        .filter_map(|res| {
            let key = res.ok()?;
            if key.1 == subscriber {
                Some(key)
            } else {
                None
            }
        })
        .take(limit)
        .filter_map(|key| {
            let mut subscription = subscriptions().may_load(deps.storage, key).ok().flatten()?;
            evaluate_subscription_status(&mut subscription, &env); // Evaluate `is_active`
            Some(subscription.subscription_id)
        })
        .collect();

    Ok(subscription_ids)
}

pub fn query_subscription_ids_for_active_subscriptions(
    deps: Deps,
    env: Env,
    start_after: Option<(String, String)>,
    limit: Option<u64>,
) -> Result<Vec<Uint128>, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(|key| Bound::exclusive(key));

    let subscription_ids = subscriptions()
        .keys(deps.storage, start, None, Order::Ascending)
        .filter_map(|res| {
            let key = res.ok()?;
            let mut subscription = subscriptions().may_load(deps.storage, key).ok().flatten()?;
            evaluate_subscription_status(&mut subscription, &env); // Evaluate `is_active`
            if subscription.is_active {
                Some(subscription.subscription_id)
            } else {
                None
            }
        })
        .take(limit)
        .collect();

    Ok(subscription_ids)
}

fn query_authorized_addresses(
    deps: Deps,
    action: PermissionAction,
    start_after: Option<String>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> Result<AuthorizedAddressesResponse, ContractError> {
    let addresses = ADOContract::default().query_permissioned_actors(
        deps,
        action.as_str(),
        start_after,
        limit,
        order_by,
    )?;
    Ok(AuthorizedAddressesResponse { addresses })
}

fn evaluate_subscription_status(
    subscription: &mut SubscriptionState,
    env: &Env,
) {
    if subscription.is_active {
        if let Expiration::AtTime(end_time) = subscription.end_time {
            if env.block.time > end_time {
                subscription.is_active = false; // Mark as inactive
                subscription.payment_pending = subscription.payment_amount;
            }
        }
    }
}