use andromeda_std::{
    amp::AndrAddr,
    andr_exec, andr_instantiate,
    common::{denom::PermissionAction, expiration::Expiry},
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;
use cw721::Cw721ReceiveMsg;

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
