use andromeda_std::error::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Order, Storage, Uint128};
use cw_storage_plus::{Bound, Index, IndexList, IndexedMap, Item, MultiIndex};
use cw_utils::Expiration;

/// Constants for pagination limits
const MAX_LIMIT: u64 = 30;
const DEFAULT_LIMIT: u64 = 10;

/// Tracks the next available subscription ID
pub const NEXT_SUBSCRIPTION_ID: Item<Uint128> = Item::new("next_subscription_id");

/// Stores the state of individual subscriptions
#[cw_serde]
pub struct SubscriptionState {
    pub subscription_id: Uint128,   // Unique subscription ID
    pub creator: String,            // Address of the content creator
    pub subscriber: String,         // Address of the subscriber
    pub token_id: String,           // NFT token ID
    pub nft_address: String,        // NFT contract address
    pub start_time: Expiration,     // Subscription start time
    pub end_time: Expiration,       // Subscription end time
    pub payment_amount: Uint128,    // Payment amount for subscription
    pub payment_pending: Uint128,   // Payment amount pending for current for this subscription
    pub payment_denom: String,      // Denomination of the payment (CW20 or native token)
    pub subscription_duration: u64, // Default subscription duration in seconds (specified by creator)
    pub is_active: bool,            // Tracks if the subscription is active
}

/// Index structure for subscriptions
pub struct SubscriptionIndices<'a> {
    /// Secondary index: subscriptions by creator address
    pub creator: MultiIndex<'a, String, SubscriptionState, (String, String)>,
}

/// Implementing indices for subscriptions
impl IndexList<SubscriptionState> for SubscriptionIndices<'_> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<SubscriptionState>> + '_> {
        let v: Vec<&dyn Index<SubscriptionState>> = vec![&self.creator];
        Box::new(v.into_iter())
    }
}

/// Indexed map to store subscriptions and their secondary indices
pub fn subscriptions<'a>(
) -> IndexedMap<'a, (String, String), SubscriptionState, SubscriptionIndices<'a>> {
    let indices = SubscriptionIndices {
        creator: MultiIndex::new(
            |_pk, subscription| subscription.creator.clone(),
            "subscriptions",
            "creator_index",
        ),
    };
    IndexedMap::new("subscriptions", indices)
}

/// Helper function to paginate and read subscriptions by creator
pub fn read_subscriptions(
    storage: &dyn Storage,
    creator: String,
    start_after: Option<(String, String)>,
    limit: Option<u64>,
) -> Result<Vec<SubscriptionState>, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let keys = subscriptions()
        .idx
        .creator
        .prefix(creator)
        .keys(storage, start, None, Order::Ascending)
        .take(limit)
        .collect::<Result<Vec<(String, String)>, _>>()?;

    let mut res = Vec::new();
    for key in keys {
        let state = subscriptions().load(storage, key)?;
        res.push(state);
    }
    Ok(res)
}

/// Helper function to fetch and increment the next subscription ID
pub fn get_and_increment_next_subscription_id(
    storage: &mut dyn Storage,
) -> Result<Uint128, ContractError> {
    let next_id = NEXT_SUBSCRIPTION_ID.load(storage)?;
    let new_id = next_id.checked_add(Uint128::from(1u128))?;
    NEXT_SUBSCRIPTION_ID.save(storage, &new_id)?;

    Ok(next_id)
}
