use cosmwasm_std::{
    testing::{mock_env, mock_info},
    from_json, to_json_binary, Addr, DepsMut, Response, Uint128,
};

use crate::{
    contract::{execute, instantiate, query},
    state::{subscriptions, SubscriptionState},
    subscription::{Cw20HookMsg, Cw721HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg},
};

pub use andromeda_std::{
    ado_base::permissioning::{LocalPermission, Permission},
    ado_contract::ADOContract,
    amp::AndrAddr,
    common::{
        context::ExecuteContext,
        denom::{SEND_CW20_ACTION, SEND_NFT_ACTION},
    },
    error::ContractError,
    testing::mock_querier::{mock_dependencies_custom, MOCK_APP_CONTRACT, MOCK_KERNEL_CONTRACT},
};

use cw20::Cw20ReceiveMsg;
use cw721::Cw721ReceiveMsg;
use cw_utils::Expiration;

fn init(
    deps: DepsMut,
    authorized_cw20_addresses: Option<Vec<AndrAddr>>,
    authorized_token_addresses: Option<Vec<AndrAddr>>,
) -> Response {
    let msg = InstantiateMsg {
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_cw20_addresses,
        authorized_token_addresses,
    };

    let info = mock_info("owner", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

#[test]
fn test_subscription_instantiate() {
    let mut deps = mock_dependencies_custom(&[]);
    let res = init(deps.as_mut(), None, None);
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_instantiate_with_multiple_authorized_cw20_addresses() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);

    let authorized_cw20_addresses = vec![
        AndrAddr::from_string("cw20_contract_1"),
        AndrAddr::from_string("cw20_contract_2"),
        AndrAddr::from_string("cw20_contract_3"),
    ];

    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        authorized_token_addresses: None,
        authorized_cw20_addresses: Some(authorized_cw20_addresses.clone()),
    };

    let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // Check if each authorized CW20 address has the correct permission
    for addr in authorized_cw20_addresses {
        let raw_addr = addr.get_raw_address(&deps.as_ref()).unwrap();
        let permission =
            ADOContract::get_permission(deps.as_ref().storage, SEND_CW20_ACTION, raw_addr).unwrap();
        assert_eq!(
            permission,
            Some(Permission::Local(LocalPermission::Whitelisted(None)))
        );
    }

    // Check that a non-authorized address doesn't have permission
    let non_authorized = "non_authorized_cw20".to_string();
    let permission =
        ADOContract::get_permission(deps.as_ref().storage, SEND_CW20_ACTION, non_authorized)
            .unwrap();
    assert_eq!(permission, None);
}

#[test]
fn test_instantiate_with_multiple_authorized_cw721_addresses() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);

    let authorized_token_addresses = vec![
        AndrAddr::from_string("cw721_contract_1"),
        AndrAddr::from_string("cw721_contract_2"),
        AndrAddr::from_string("cw721_contract_3"),
    ];

    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        authorized_cw20_addresses: None,
        authorized_token_addresses: Some(authorized_token_addresses.clone()),
    };

    let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // Check if each authorized CW721 address has the correct permission
    for addr in authorized_token_addresses {
        let raw_addr = addr.get_raw_address(&deps.as_ref()).unwrap();
        let permission =
            ADOContract::get_permission(deps.as_ref().storage, SEND_NFT_ACTION, raw_addr).unwrap();
        assert_eq!(
            permission,
            Some(Permission::Local(LocalPermission::Whitelisted(None)))
        );
    }

    // Check that a non-authorized address doesn't have permission
    let non_authorized = "non_authorized_cw721".to_string();
    let permission =
        ADOContract::get_permission(deps.as_ref().storage, SEND_NFT_ACTION, non_authorized)
            .unwrap();
    assert_eq!(permission, None);
}

#[test]
fn test_instantiate_with_owner_set() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);

    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: Some("new_owner".to_string()),
        authorized_token_addresses: None,
        authorized_cw20_addresses: None,
    };

    let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // Verify that the owner is set correctly
    let ado_contract = ADOContract::default(); // Create an instance of ADOContract
    let owner = ado_contract.owner(deps.as_ref().storage).unwrap();
    assert_eq!(owner, Addr::unchecked("new_owner"));
}

#[test]
fn test_execute_subscribe_success() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let cw20_address = "authorized_cw20".to_string();

    // Initialize the contract with the CW20 address authorized
    let msg = InstantiateMsg {
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_cw20_addresses: Some(vec![AndrAddr::from_string(&cw20_address)]),
        authorized_token_addresses: None,
    };

    let info = mock_info("owner", &[]);
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Mock a subscription offering
    let creator = "creator".to_string();
    let subscriber = "".to_string(); // No subscriber initially
    let token_id = "token_1".to_string();
    let nft_address = cw20_address.clone(); // Use the authorized CW20 address
    let payment_amount = Uint128::from(100u128);
    let duration = 3600;

    let offering = SubscriptionState {
        subscription_id: Uint128::from(1u128),
        creator: creator.clone(),
        subscriber: subscriber.clone(),
        token_id: token_id.clone(),
        nft_address: nft_address.clone(),
        start_time: Expiration::Never {},
        end_time: Expiration::Never {},
        payment_amount,
        payment_pending: payment_amount,
        payment_denom: "CW20".to_string(),
        subscription_duration: duration,
        is_active: false,
    };
    subscriptions()
        .save(
            deps.as_mut().storage,
            (nft_address.clone(), subscriber.clone()),
            &offering,
        )
        .unwrap();

    // Define the Cw20ReceiveMsg
    let receive_msg = Cw20ReceiveMsg {
        sender: "user".to_string(),
        amount: payment_amount,
        msg: to_json_binary(&Cw20HookMsg::Subscribe {
            token_id: token_id.clone(),
            nft_address: nft_address.clone(),
        })
        .unwrap(),
    };

    let msg = ExecuteMsg::Receive(receive_msg);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Validate the response
    assert_eq!(res.attributes.len(), 7);
    assert_eq!(res.attributes[0].value, "subscribe");
    assert_eq!(res.attributes[1].value, "user");
    assert_eq!(res.attributes[2].value, "creator");
    assert_eq!(res.attributes[3].value, cw20_address);
    assert_eq!(res.attributes[4].key, "start_time");
    assert_eq!(res.attributes[5].key, "end_time");
    assert_eq!(res.attributes[6].key, "is_active");
    assert_eq!(res.attributes[6].value, "true");

    // Validate the state
    let saved_subscription = subscriptions()
        .load(
            deps.as_ref().storage,
            (nft_address.clone(), "user".to_string()),
        )
        .unwrap();
    assert!(saved_subscription.is_active);
    assert_eq!(
        saved_subscription.start_time,
        Expiration::AtTime(env.block.time)
    );
    assert_eq!(
        saved_subscription.end_time,
        Expiration::AtTime(env.block.time.plus_seconds(duration))
    );
}

#[test]
fn test_execute_renew_success() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let cw20_address = "authorized_cw20".to_string();

    // Initialize the contract with the CW20 address authorized
    let msg = InstantiateMsg {
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_cw20_addresses: Some(vec![AndrAddr::from_string(&cw20_address)]),
        authorized_token_addresses: None,
    };

    let owner_info = mock_info("owner", &[]);
    instantiate(deps.as_mut(), env.clone(), owner_info.clone(), msg).unwrap();

    // Mock an active subscription
    let creator = "creator".to_string();
    let subscriber = "user".to_string();
    let token_id = "token_1".to_string();
    let nft_address = cw20_address.clone();
    let payment_amount = Uint128::from(100u128);
    let duration = 3600;

    let subscription = SubscriptionState {
        subscription_id: Uint128::from(1u128),
        creator: creator.clone(),
        subscriber: subscriber.clone(),
        token_id: token_id.clone(),
        nft_address: nft_address.clone(),
        start_time: Expiration::AtTime(env.block.time),
        end_time: Expiration::AtTime(env.block.time.plus_seconds(duration)),
        payment_amount,
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: duration,
        is_active: false,
    };

    subscriptions()
        .save(
            deps.as_mut().storage,
            (nft_address.clone(), subscriber.clone()),
            &subscription,
        )
        .unwrap();

    // Define the Cw20ReceiveMsg for renewal
    let receive_msg = Cw20ReceiveMsg {
        sender: "user".to_string(),
        amount: payment_amount,
        msg: to_json_binary(&Cw20HookMsg::Renew {
            token_id: token_id.clone(),
            nft_address: nft_address.clone(),
        })
        .unwrap(),
    };

    // Define the ExecuteMsg
    let msg = ExecuteMsg::Receive(receive_msg);

    // Mock info with the CW20 contract as sender
    let cw20_info = mock_info(&cw20_address, &[]);

    // Execute the handler
    let res = execute(deps.as_mut(), env.clone(), cw20_info.clone(), msg).unwrap();

    // Validate the response attributes
    assert_eq!(res.attributes.len(), 8);
    assert_eq!(res.attributes[0].value, "renew_subscription");
    assert_eq!(res.attributes[1].value, "user");
    assert_eq!(res.attributes[2].value, "creator");
    assert_eq!(res.attributes[3].key, "creator address");
    assert_eq!(res.attributes[7].value, "true");

    // Validate the updated state
    let renewed_subscription = subscriptions()
        .load(
            deps.as_ref().storage,
            (nft_address.clone(), subscriber.clone()),
        )
        .unwrap();

    assert!(renewed_subscription.is_active);
    assert_eq!(
        renewed_subscription.start_time,
        Expiration::AtTime(env.block.time)
    );
    assert_eq!(
        renewed_subscription.end_time,
        Expiration::AtTime(env.block.time.plus_seconds(duration))
    );
    assert_eq!(renewed_subscription.payment_pending, Uint128::zero());
}

#[test]
fn test_execute_receive_cw721_success() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    // Initialize the contract with an authorized CW721 address
    let cw721_address = "authorized_cw721".to_string();
    let msg = InstantiateMsg {
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_cw20_addresses: None,
        authorized_token_addresses: Some(vec![AndrAddr::from_string(&cw721_address)]),
    };

    let owner_info = mock_info("owner", &[]);
    instantiate(deps.as_mut(), env.clone(), owner_info, msg).unwrap();

    // Define the Cw721ReceiveMsg
    let creator = "creator".to_string();
    let token_id = "token_1".to_string();
    let payment_amount = Uint128::from(100u128);
    let duration = 3600;

    let hook_msg = Cw721HookMsg::RegisterSubscription {
        duration,
        payment_amount,
    };

    let receive_msg = Cw721ReceiveMsg {
        sender: creator.clone(),
        token_id: token_id.clone(),
        msg: to_json_binary(&hook_msg).unwrap(),
    };

    let execute_msg = ExecuteMsg::ReceiveNft(receive_msg);

    // Mock info with the CW721 contract as sender
    let cw721_info = mock_info(&cw721_address, &[]);

    // Execute the handler
    let res = execute(deps.as_mut(), env.clone(), cw721_info.clone(), execute_msg).unwrap();

    // Validate the response attributes
    assert_eq!(res.attributes.len(), 6);
    assert_eq!(res.attributes[0].value, "register_subscription");
    assert_eq!(res.attributes[1].value, creator);

    // Validate the state
    let saved_subscription = subscriptions()
        .load(
            deps.as_ref().storage,
            (cw721_address.clone(), "".to_string()),
        )
        .unwrap();
    assert!(!saved_subscription.is_active);
    assert_eq!(saved_subscription.token_id, token_id);
    assert_eq!(saved_subscription.payment_amount, payment_amount);
    assert_eq!(saved_subscription.subscription_duration, duration);
}
#[test]
fn test_execute_receive_cw721_duplicate_registration() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let cw721_address = "authorized_cw721".to_string();
    let msg = InstantiateMsg {
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_cw20_addresses: None,
        authorized_token_addresses: Some(vec![AndrAddr::from_string(&cw721_address)]),
    };

    let owner_info = mock_info("owner", &[]);
    instantiate(deps.as_mut(), env.clone(), owner_info, msg).unwrap();

    let creator = "creator".to_string();
    let token_id = "token_1".to_string();
    let payment_amount = Uint128::from(100u128);
    let duration = 3600;

    let hook_msg = Cw721HookMsg::RegisterSubscription {
        duration,
        payment_amount,
    };

    let receive_msg = Cw721ReceiveMsg {
        sender: creator.clone(),
        token_id: token_id.clone(),
        msg: to_json_binary(&hook_msg).unwrap(),
    };

    let execute_msg = ExecuteMsg::ReceiveNft(receive_msg.clone());

    let cw721_info = mock_info(&cw721_address, &[]);

    // Execute the handler for the first time (success)
    execute(
        deps.as_mut(),
        env.clone(),
        cw721_info.clone(),
        execute_msg.clone(),
    )
    .unwrap();

    // Attempt to register the same subscription again
    let err = execute(
        deps.as_mut(),
        env.clone(),
        cw721_info.clone(),
        execute_msg.clone(),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ContractError::CustomError {
            msg: "Subscription offering already exists for this NFT.".to_string(),
        }
    );
}

#[test]
fn test_execute_cancel_success() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let subscriber_info = mock_info("subscriber", &[]);

    let msg = InstantiateMsg {
        owner: Some("owner".to_string()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_cw20_addresses: None,
        authorized_token_addresses: None,
    };

    let owner_info = mock_info("owner", &[]);
    instantiate(deps.as_mut(), env.clone(), owner_info.clone(), msg).unwrap();

    let creator = "creator".to_string();
    let subscriber = subscriber_info.sender.to_string();
    let nft_address = "nft_contract".to_string();
    let token_id = "token_1".to_string();

    let subscription = SubscriptionState {
        subscription_id: Uint128::from(1u128),
        creator: creator.clone(),
        subscriber: subscriber.clone(),
        token_id: token_id.clone(),
        nft_address: nft_address.clone(),
        start_time: Expiration::AtTime(env.block.time),
        end_time: Expiration::AtTime(env.block.time.plus_seconds(3600)), // 1 hour later
        payment_amount: Uint128::from(100u128),
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: 3600,
        is_active: true,
    };

    // Save the subscription in state
    subscriptions()
        .save(
            deps.as_mut().storage,
            (nft_address.clone(), subscriber.clone()),
            &subscription,
        )
        .unwrap();

    // Define the `Cancel` ExecuteMsg
    let msg = ExecuteMsg::Cancel {
        nft_address: nft_address.clone(),
    };

    // Execute the cancel operation
    let res = execute(deps.as_mut(), env.clone(), subscriber_info.clone(), msg).unwrap();

    // Validate the response
    assert_eq!(res.attributes.len(), 5);
    assert_eq!(res.attributes[0].value, "cancel_subscription");
    assert_eq!(res.attributes[1].value, creator);
    assert_eq!(res.attributes[2].value, subscriber);
    assert_eq!(res.attributes[3].value, "false");

    // Validate the state after cancellation
    let cancelled_subscription = subscriptions()
        .load(
            deps.as_ref().storage,
            (nft_address.clone(), subscriber.clone()),
        )
        .unwrap();

    assert!(!cancelled_subscription.is_active);
    assert_eq!(cancelled_subscription.start_time, Expiration::Never {});
    assert_eq!(cancelled_subscription.end_time, Expiration::Never {});
    assert_eq!(
        cancelled_subscription.payment_pending,
        subscription.payment_amount
    );
}

#[test]
fn test_execute_cancel_failure_no_subscription() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let subscriber_info = mock_info("subscriber", &[]);

    let msg = InstantiateMsg {
        owner: Some("owner".to_string()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_cw20_addresses: None,
        authorized_token_addresses: None,
    };

    let owner_info = mock_info("owner", &[]);
    instantiate(deps.as_mut(), env.clone(), owner_info.clone(), msg).unwrap();

    // Define the `Cancel` ExecuteMsg with no subscription in state
    let nft_address = "nft_contract".to_string();
    let msg = ExecuteMsg::Cancel {
        nft_address: nft_address.clone(),
    };
    let err = execute(deps.as_mut(), env.clone(), subscriber_info.clone(), msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::CustomError {
            msg: format!(
                "No subscription found for address {} and subscriber {}.",
                nft_address, subscriber_info.sender
            )
        }
    );
}

#[test]
fn test_query_subscription_success() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let creator = "creator".to_string();
    let subscriber = "subscriber".to_string();
    let token_id = "token_1".to_string();
    let nft_address = "nft_contract".to_string();
    let payment_amount = Uint128::from(100u128);
    let duration = 3600; 

    let subscription = SubscriptionState {
        subscription_id: Uint128::from(1u128),
        creator: creator.clone(),
        subscriber: subscriber.clone(),
        token_id: token_id.clone(),
        nft_address: nft_address.clone(),
        start_time: Expiration::AtTime(env.block.time),
        end_time: Expiration::AtTime(env.block.time.plus_seconds(duration)),
        payment_amount,
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: duration,
        is_active: true,
    };

    subscriptions()
        .save(
            deps.as_mut().storage,
            (creator.clone(), subscriber.clone()),
            &subscription,
        )
        .unwrap();

    let query_msg = QueryMsg::Subscription {
        creator: creator.clone(),
        subscriber: subscriber.clone(),
    };

    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let queried_subscription: SubscriptionState = from_json(&res).unwrap();

    assert_eq!(queried_subscription.subscription_id, subscription.subscription_id);
    assert_eq!(queried_subscription.creator, creator);
    assert_eq!(queried_subscription.subscriber, subscriber);
    assert_eq!(queried_subscription.is_active, true);
    assert_eq!(queried_subscription.payment_amount, payment_amount);
    assert_eq!(queried_subscription.start_time, subscription.start_time);
    assert_eq!(queried_subscription.end_time, subscription.end_time);
}

#[test]
fn test_query_subscriptions_for_creator_success() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let creator = "creator".to_string();
    let subscriber_1 = "subscriber_1".to_string();
    let subscriber_2 = "subscriber_2".to_string();
    let token_id = "token_1".to_string();
    let nft_address = "nft_contract".to_string();
    let payment_amount = Uint128::from(100u128);
    let duration = 3600; 

    let subscription_1 = SubscriptionState {
        subscription_id: Uint128::from(1u128),
        creator: creator.clone(),
        subscriber: subscriber_1.clone(),
        token_id: token_id.clone(),
        nft_address: nft_address.clone(),
        start_time: Expiration::AtTime(env.block.time),
        end_time: Expiration::AtTime(env.block.time.plus_seconds(duration)),
        payment_amount,
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: duration,
        is_active: true,
    };

    let subscription_2 = SubscriptionState {
        subscription_id: Uint128::from(2u128),
        creator: creator.clone(),
        subscriber: subscriber_2.clone(),
        token_id: token_id.clone(),
        nft_address: nft_address.clone(),
        start_time: Expiration::AtTime(env.block.time),
        end_time: Expiration::AtTime(env.block.time.plus_seconds(duration)),
        payment_amount,
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: duration,
        is_active: true,
    };

    subscriptions()
        .save(
            deps.as_mut().storage,
            (creator.clone(), subscriber_1.clone()),
            &subscription_1,
        )
        .unwrap();

    subscriptions()
        .save(
            deps.as_mut().storage,
            (creator.clone(), subscriber_2.clone()),
            &subscription_2,
        )
        .unwrap();

    let query_msg = QueryMsg::SubscriptionsForCreator {
        creator: creator.clone(),
        start_after: None,
        limit: Some(10),
    };

    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let queried_subscriptions: Vec<SubscriptionState> = from_json(&res).unwrap();

    assert_eq!(queried_subscriptions.len(), 2);
    assert_eq!(queried_subscriptions[0].subscriber, subscriber_1);
    assert_eq!(queried_subscriptions[1].subscriber, subscriber_2);
}

#[test]
fn test_query_subscription_ids_for_active_subscriptions() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let creator = "creator".to_string();
    let subscriber_1 = "subscriber_1".to_string();
    let subscriber_2 = "subscriber_2".to_string();
    let token_id = "token_1".to_string();
    let nft_address = "nft_contract".to_string();
    let payment_amount = Uint128::from(100u128);
    let duration = 3600;

    let active_subscription = SubscriptionState {
        subscription_id: Uint128::from(1u128),
        creator: creator.clone(),
        subscriber: subscriber_1.clone(),
        token_id: token_id.clone(),
        nft_address: nft_address.clone(),
        start_time: Expiration::AtTime(env.block.time),
        end_time: Expiration::AtTime(env.block.time.plus_seconds(duration)),
        payment_amount,
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: duration,
        is_active: true,
    };

    let inactive_subscription = SubscriptionState {
        subscription_id: Uint128::from(2u128),
        creator: creator.clone(),
        subscriber: subscriber_2.clone(),
        token_id: token_id.clone(),
        nft_address: nft_address.clone(),
        start_time: Expiration::AtTime(env.block.time),
        end_time: Expiration::AtTime(env.block.time.minus_seconds(duration)),
        payment_amount,
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: duration,
        is_active: false,
    };

    subscriptions()
        .save(
            deps.as_mut().storage,
            (creator.clone(), subscriber_1.clone()),
            &active_subscription,
        )
        .unwrap();

    subscriptions()
        .save(
            deps.as_mut().storage,
            (creator.clone(), subscriber_2.clone()),
            &inactive_subscription,
        )
        .unwrap();

    let query_msg = QueryMsg::SubscriptionIdsForActiveSubscriptions {
        start_after: None,
        limit: Some(10),
    };

    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let queried_ids: Vec<Uint128> = from_json(&res).unwrap();

    assert_eq!(queried_ids.len(), 1);
    assert_eq!(queried_ids[0], active_subscription.subscription_id);
}

#[test]
fn test_query_active_subscription_ids() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    // Add two active subscriptions and one expired subscription to the state
    let active_subscription_1 = SubscriptionState {
        subscription_id: Uint128::from(1u128),
        creator: "creator_1".to_string(),
        subscriber: "subscriber_1".to_string(),
        token_id: "token_1".to_string(),
        nft_address: "nft_address_1".to_string(),
        start_time: Expiration::AtTime(env.block.time.minus_seconds(100)),
        end_time: Expiration::AtTime(env.block.time.plus_seconds(100)), // Active
        payment_amount: Uint128::from(100u128),
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: 200,
        is_active: true,
    };

    let active_subscription_2 = SubscriptionState {
        subscription_id: Uint128::from(2u128),
        creator: "creator_2".to_string(),
        subscriber: "subscriber_2".to_string(),
        token_id: "token_2".to_string(),
        nft_address: "nft_address_2".to_string(),
        start_time: Expiration::AtTime(env.block.time.minus_seconds(200)),
        end_time: Expiration::AtTime(env.block.time.plus_seconds(50)), // Active
        payment_amount: Uint128::from(200u128),
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: 250,
        is_active: true,
    };

    let expired_subscription = SubscriptionState {
        subscription_id: Uint128::from(3u128),
        creator: "creator_3".to_string(),
        subscriber: "subscriber_3".to_string(),
        token_id: "token_3".to_string(),
        nft_address: "nft_address_3".to_string(),
        start_time: Expiration::AtTime(env.block.time.minus_seconds(300)),
        end_time: Expiration::AtTime(env.block.time.minus_seconds(100)), // Expired
        payment_amount: Uint128::from(300u128),
        payment_pending: Uint128::zero(),
        payment_denom: "CW20".to_string(),
        subscription_duration: 200,
        is_active: true,
    };

    subscriptions()
        .save(
            deps.as_mut().storage,
            (
                active_subscription_1.nft_address.clone(),
                active_subscription_1.subscriber.clone(),
            ),
            &active_subscription_1,
        )
        .unwrap();

    subscriptions()
        .save(
            deps.as_mut().storage,
            (
                active_subscription_2.nft_address.clone(),
                active_subscription_2.subscriber.clone(),
            ),
            &active_subscription_2,
        )
        .unwrap();

    subscriptions()
        .save(
            deps.as_mut().storage,
            (
                expired_subscription.nft_address.clone(),
                expired_subscription.subscriber.clone(),
            ),
            &expired_subscription,
        )
        .unwrap();

    let query_msg = QueryMsg::SubscriptionIdsForActiveSubscriptions {
        start_after: None,
        limit: None,
    };

    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let active_ids: Vec<Uint128> = from_json(&res).unwrap();

    // Assert only the active subscription IDs are returned
    assert_eq!(active_ids.len(), 2);
    assert!(active_ids.contains(&Uint128::from(1u128)));
    assert!(active_ids.contains(&Uint128::from(2u128)));
    assert!(!active_ids.contains(&Uint128::from(3u128)));
}