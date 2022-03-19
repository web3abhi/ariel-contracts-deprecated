use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetUserResponse {
    pub collateral: u128,
    pub cumulative_deposits: i128,
    pub total_fee_paid: u128,
    pub total_token_discount: u128,
    pub total_referral_reward: u128,
    pub total_referee_discount: u128,
    pub positions_length: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetUserPositionResponse {
    pub base_asset_amount: i128,
    pub quote_asset_amount: u128,
    pub last_cumulative_funding_rate: i128,
    pub last_cumulative_repeg_rebate: u128,
    pub last_funding_rate_ts: i64,
    pub stop_loss_price: u128,
    pub stop_loss_amount: u128,
    pub stop_profit_price: u128,
    pub stop_profit_amount: u128,
    pub transfer_to: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetAdminResponse {
    pub admin: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetIsExchangePausedResponse {
    pub exchange_paused: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetIsFundingPausedResponse {
    pub funding_paused: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetAdminControlsPricesResponse {
    pub admin_controls_prices: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetVaultsResponse {
    pub insurance_vault: String,
    pub collateral_vault: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetMarginRatioResponse {
    pub margin_ratio_initial: u128,
    pub margin_ratio_maintenance: u128,
    pub margin_ratio_partial: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetPartialLiquidationClosePercentageResponse {
    pub numerator: u128,
    pub denominator: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetPartialLiquidationPenaltyPercentageResponse {
    pub numerator: u128,
    pub denominator: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetFullLiquidationPenaltyPercentageResponse {
    pub numerator: u128,
    pub denominator: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetPartialLiquidatorSharePercentageResponse {
    pub denominator: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetFullLiquidatorSharePercentageResponse {
    pub denominator: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetMaxDepositLimitResponse {
    pub max_deposit: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetFeeStructureResponse {
    pub fee_numerator: u128,
    pub fee_denominator: u128,
    pub t1_minimum_balance: u64,
    pub t1_discount_numerator: u128,
    pub t1_discount_denominator: u128,

    pub t2_minimum_balance: u64,
    pub t2_discount_numerator: u128,
    pub t2_discount_denominator: u128,

    pub t3_minimum_balance: u64,
    pub t3_discount_numerator: u128,
    pub t3_discount_denominator: u128,

    pub referrer_reward_numerator: u128,
    pub referrer_reward_denominator: u128,
    pub referee_discount_numerator: u128,
    pub referee_discount_denominator: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetCurveHistoryLengthResponse {
    pub length: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetCurveHistoryResponse {
    pub ts: i64,
    pub record_id: u128,
    pub market_index: u64,
    pub peg_multiplier_before: u128,
    pub base_asset_reserve_before: u128,
    pub quote_asset_reserve_before: u128,
    pub sqrt_k_before: u128,
    pub peg_multiplier_after: u128,
    pub base_asset_reserve_after: u128,
    pub quote_asset_reserve_after: u128,
    pub sqrt_k_after: u128,
    pub base_asset_amount_long: u128,
    pub base_asset_amount_short: u128,
    pub base_asset_amount: i128,
    pub open_interest: u128,
    pub total_fee: u128,
    pub total_fee_minus_distributions: u128,
    pub adjustment_cost: i128,
    pub oracle_price: i128,
    pub trade_record: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetDepositHistoryLengthResponse {
    pub length: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetDepositHistoryResponse {
    pub ts: i64,
    pub record_id: u128,
    pub user: String,
    pub direction: String,
    pub collateral_before: u128,
    pub cumulative_deposits_before: i128,
    pub amount: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetFundingPaymentHistoryLengthResponse {
    pub length: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetFundingPaymentHistoryResponse {
    pub ts: i64,
    pub record_id: usize,
    pub user: String,
    pub market_index: u64,
    pub funding_payment: i128,
    pub base_asset_amount: i128,
    pub user_last_cumulative_funding: i128,
    pub user_last_funding_rate_ts: i64,
    pub amm_cumulative_funding_long: i128,
    pub amm_cumulative_funding_short: i128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetFundingRateHistoryLengthResponse {
    pub length: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetFundingRateHistoryResponse {
    pub ts: i64,
    pub record_id: usize,
    pub market_index: u64,
    pub funding_rate: i128,
    pub cumulative_funding_rate_long: i128,
    pub cumulative_funding_rate_short: i128,
    pub oracle_price_twap: i128,
    pub mark_price_twap: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetLiquidationHistoryLengthResponse {
    pub length: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetLiquidationHistoryResponse {
    pub ts: i64,
    pub record_id: u128,
    pub user: String,
    pub partial: bool,
    pub base_asset_value: u128,
    pub base_asset_value_closed: u128,
    pub liquidation_fee: u128,
    pub fee_to_liquidator: u64,
    pub fee_to_insurance_fund: u64,
    pub liquidator: String,
    pub total_collateral: u128,
    pub collateral: u128,
    pub unrealized_pnl: i128,
    pub margin_ratio: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetTradeHistoryLengthResponse {
    pub length: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetTradeHistoryResponse {
    pub ts: i64,
    pub record_id: u128,
    pub user: String,
    pub direction: String,
    pub base_asset_amount: u128,
    pub quote_asset_amount: u128,
    pub mark_price_before: u128,
    pub mark_price_after: u128,
    pub fee: u128,
    pub referrer_reward: u128,
    pub referee_discount: u128,
    pub token_discount: u128,
    pub liquidation: bool,
    pub market_index: u64,
    pub oracle_price: i128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetMarketInfoResponse {
    pub market_name: String,
    pub initialized: bool,
    pub base_asset_amount_long: i128,
    pub base_asset_amount_short: i128,
    pub base_asset_amount: i128, // net market bias
    pub open_interest: u128,  
    pub oracle: String,
    pub oracle_source: String,
    pub base_asset_reserve: u128,
    pub quote_asset_reserve: u128,
    pub cumulative_repeg_rebate_long: u128,
    pub cumulative_repeg_rebate_short: u128,
    pub cumulative_funding_rate_long: i128,
    pub cumulative_funding_rate_short: i128,
    pub last_funding_rate: i128,
    pub last_funding_rate_ts: i64,
    pub funding_period: i64,
    pub last_oracle_price_twap: i128,
    pub last_mark_price_twap: u128,
    pub last_mark_price_twap_ts: i64,
    pub sqrt_k: u128,
    pub peg_multiplier: u128,
    pub total_fee: u128,
    pub total_fee_minus_distributions: u128,
    pub total_fee_withdrawn: u128,
    pub minimum_trade_size: u128,
    pub last_oracle_price_twap_ts: i64,
    pub last_oracle_price: i128,
}

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// pub struct Response {
//     pub length: u64,
// }