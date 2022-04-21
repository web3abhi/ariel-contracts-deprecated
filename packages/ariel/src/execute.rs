use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::{OracleSource, OrderParams, PositionDirection};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub collateral_vault: String,
    pub insurance_vault: String,
    pub admin_controls_prices: bool,
    pub oracle: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // market initializer updates AMM structure
    InitializeMarket {
        market_index: u64,
        market_name: String,
        amm_base_asset_reserve: u128,
        amm_quote_asset_reserve: u128,
        amm_periodicity: u64,
        amm_peg_multiplier: u128,
        oracle_source: OracleSource,
        margin_ratio_initial: u32,
        margin_ratio_partial: u32,
        margin_ratio_maintenance: u32,
    },
    //deposit collateral, updates user struct
    DepositCollateral {
        amount: u64,
        referrer: Option<String>
    },
    //user function withdraw collateral, updates user struct
    WithdrawCollateral {
        amount: u64,
    },
    OpenPosition {
        direction: PositionDirection,
        quote_asset_amount: u128,
        market_index: u64,
        limit_price: u128,
    },
    ClosePosition {
        market_index: u64,
    },

    // order related messages
    PlaceOrder {
        order: OrderParams,
    },
    CancelOrder {
        market_index: u64,
        order_id: u64,
    },
    ExpireOrders {
        user_address: String,
    },
    FillOrder {
        order_id: u64,
        user_address: String,
        market_index: u64,
    },
    Liquidate {
        user: String,
        market_index: u64,
    },
    MoveAMMPrice {
        base_asset_reserve: u128,
        quote_asset_reserve: u128,
        market_index: u64,
    },
    //user function
    WithdrawFees {
        market_index: u64,
        amount: u64,
    },

    // withdraw from insurance vault sends token but no logic

    //admin function
    WithdrawFromInsuranceVaultToMarket {
        market_index: u64,
        amount: u64,
    },
    //admin function
    RepegAMMCurve {
        new_peg_candidate: u128,
        market_index: u64,
    },

    UpdateAMMOracleTwap {
        market_index: u64,
    },

    ResetAMMOracleTwap {
        market_index: u64,
    },
    //user calls it we get the user identification from msg address sender
    SettleFundingPayment {},
    UpdateFundingRate {
        market_index: u64,
    },
    UpdateK {
        market_index: u64,
        sqrt_k: u128,
    },
    UpdateMarginRatio {
        market_index: u64,
        margin_ratio_initial: u32,
        margin_ratio_partial: u32,
        margin_ratio_maintenance: u32,
    },
    UpdatePartialLiquidationClosePercentage {
        numerator: u128,
        denominator: u128,
    },
    UpdatePartialLiquidationPenaltyPercentage {
        numerator: u128,
        denominator: u128,
    },
    UpdateFullLiquidationPenaltyPercentage {
        numerator: u128,
        denominator: u128,
    },
    UpdatePartialLiquidationLiquidatorShareDenominator {
        denominator: u64,
    },
    UpdateFullLiquidationLiquidatorShareDenominator {
        denominator: u64,
    },
    UpdateFee {
        fee_numerator: u128,
        fee_denominator: u128,
        first_tier_minimum_balance: u64,
        first_tier_discount_numerator: u128,
        first_tier_discount_denominator: u128,
        second_tier_minimum_balance: u64,
        second_tier_discount_numerator: u128,
        second_tier_discount_denominator: u128,
        third_tier_minimum_balance: u64,
        third_tier_discount_numerator: u128,
        third_tier_discount_denominator: u128,
        fourth_tier_minimum_balance: u64,
        fourth_tier_discount_numerator: u128,
        fourth_tier_discount_denominator: u128,
        referrer_reward_numerator: u128,
        referrer_reward_denominator: u128,
        referee_discount_numerator: u128,
        referee_discount_denominator: u128,
    },
    UpdateOraceGuardRails {
        use_for_liquidations: bool,
        mark_oracle_divergence_numerator: u128,
        mark_oracle_divergence_denominator: u128,
        slots_before_stale: i64,
        confidence_interval_max_size: u128,
        too_volatile_ratio: i128,
    },
    UpdateOrderState {
        min_order_quote_asset_amount: u128,
        reward_numerator: u128,
        reward_denominator: u128,
        time_based_reward_lower_bound: u128,
    },
    UpdateMarketOracle {
        market_index: u64,
        oracle: String,
        oracle_source: OracleSource,
    },
    UpdateOracleAddress {
        oracle: String,
    },
    UpdateMarketMinimumQuoteAssetTradeSize {
        market_index: u64,
        minimum_trade_size: u128,
    },

    UpdateMarketMinimumBaseAssetTradeSize {
        market_index: u64,
        minimum_trade_size: u128,
    },
    // will move to admin controller
    UpdateAdmin {
        admin: String,
    },
    UpdateMaxDeposit {
        max_deposit: u128,
    },
    UpdateExchangePaused {
        exchange_paused: bool,
    },
    DisableAdminControlsPrices {},
    UpdateFundingPaused {
        funding_paused: bool,
    },
    OracleFeeder {
        market_index: u64,
        price: i128,
    },
}
