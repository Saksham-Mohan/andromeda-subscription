use andromeda_std::{
    amp::AndrAddr,
    andr_exec, andr_instantiate, andr_query,
    common::{
        denom::{AuthorizedAddressesResponse, PermissionAction},
        expiration::Expiry,
        OrderBy,
    },
};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;
use cw721::Cw721ReceiveMsg;

use crate::state::SubscriptionState;

#[andr_instantiate]
#[cw_serde]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub authorized_cw20_addresses: Option<Vec<AndrAddr>>,
    pub authorized_token_addresses: Option<Vec<AndrAddr>>,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    /// Message to handle CW20 token transfers.
    Receive(Cw20ReceiveMsg),
    /// Message to handle CW721 NFT transfers.
    ReceiveNft(Cw721ReceiveMsg),
    /// Cancel an existing subscription.
    Cancel { nft_address: String },
    /// Restricted to owner.
    AuthorizeContract {
        action: PermissionAction,
        addr: AndrAddr,
        expiration: Option<Expiry>,
    },
    /// Restricted to owner
    DeauthorizeContract {
        action: PermissionAction,
        addr: AndrAddr,
    },
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(SubscriptionState)]
    /// Gets the details of a specific subscription using the creator and subscriber composite key.
    Subscription { creator: String, subscriber: String },
    #[returns(Vec<SubscriptionState>)]
    /// Gets all subscriptions for a specific creator, with optional pagination.
    SubscriptionsForCreator {
        creator: String,
        start_after: Option<(String, String)>, // Composite key
        limit: Option<u64>,
    },
    #[returns(Vec<SubscriptionState>)]
    /// Gets all subscriptions for a specific subscriber, with optional pagination.
    SubscriptionsForSubscriber {
        subscriber: String,
        start_after: Option<(String, String)>, // Composite key
        limit: Option<u64>,
    },
    #[returns(Vec<Uint128>)]
    /// Gets all subscription IDs for a specific creator, with optional pagination.
    SubscriptionIdsForCreator {
        creator: String,
        start_after: Option<(String, String)>, // Composite key
        limit: Option<u64>,
    },
    #[returns(Vec<Uint128>)]
    /// Gets all subscription IDs for a specific subscriber, with optional pagination.
    SubscriptionIdsForSubscriber {
        subscriber: String,
        start_after: Option<(String, String)>, // Composite key
        limit: Option<u64>,
    },
    #[returns(Vec<Uint128>)]
    /// Gets all active subscription IDs, with optional pagination.
    SubscriptionIdsForActiveSubscriptions {
        start_after: Option<(String, String)>, // Composite key
        limit: Option<u64>,
    },
    #[returns(AuthorizedAddressesResponse)]
    /// Gets the authorized addresses for a given action.
    AuthorizedAddresses {
        action: PermissionAction,
        start_after: Option<String>,
        limit: Option<u32>,
        order_by: Option<OrderBy>,
    },
}

#[cw_serde]
pub enum Cw20HookMsg {
    Subscribe {
        /// The NFT token ID to associate with this subscription
        token_id: String,
        /// The NFT contract address that issued the token
        nft_address: String,
    },
    Renew {
        /// The NFT token ID to associate with this subscription
        token_id: String,
        /// The NFT contract address that issued the token
        nft_address: String,
    },
}

#[cw_serde]
pub enum Cw721HookMsg {
    RegisterSubscription {
        duration: u64,
        payment_amount: Uint128,
    },
}
