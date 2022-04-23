use std::ops::Div;

use crate::controller;
use crate::helpers;
use crate::helpers::constants::*;
use crate::helpers::withdrawal::calculate_withdrawal_amounts;
use crate::states::curve_history::*;
use crate::ContractError;

use crate::states::deposit_history::*;
use crate::states::liquidation_history::LiquidationInfo;
use crate::states::liquidation_history::LiquidationRecord;
use crate::states::liquidation_history::LIQUIDATION_HISTORY;
use crate::states::liquidation_history::LIQUIDATION_HISTORY_INFO;
use crate::states::market::LiquidationStatus;
use crate::states::market::LiquidationType;
use crate::states::market::{Amm, Market, MARKETS};
use crate::states::order::OrderState;
use crate::states::state::State;
use crate::states::state::ADMIN;
use crate::states::state::FEESTRUCTURE;
use crate::states::state::ORACLEGUARDRAILS;
use crate::states::state::ORDERSTATE;
use crate::states::state::STATE;
use crate::states::trade_history::TradeInfo;
use crate::states::trade_history::TradeRecord;
use crate::states::trade_history::TRADE_HISTORY;
use crate::states::trade_history::TRADE_HISTORY_INFO;
use crate::states::user::{User, POSITIONS, USERS};

use ariel::helper::addr_validate_to_lower;
use ariel::helper::assert_sent_uusd_balance;
use ariel::helper::query_balance;
use ariel::helper::VaultInterface;
use ariel::number::Number128;
use ariel::types::OraclePriceData;
use ariel::types::OrderType;
use ariel::types::{
    DepositDirection, FeeStructure, OracleGuardRails, OracleSource, OrderParams, PositionDirection,
};
use cosmwasm_std::{
    coins, to_binary, CosmosMsg, Decimal, DepsMut, Env, Fraction, MessageInfo, Response, Uint128,
    WasmMsg,
};

pub fn try_initialize_market(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    market_index: u64,
    market_name: String,
    amm_base_asset_reserve: Uint128,
    amm_quote_asset_reserve: Uint128,
    amm_periodicity: u64,
    amm_peg_multiplier: Uint128,
    oracle_source: OracleSource,
    margin_ratio_initial: u32,
    margin_ratio_partial: u32,
    margin_ratio_maintenance: u32,
) -> Result<Response, ContractError> {
    // ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let now = env.block.time.seconds();

    let state = STATE.load(deps.storage)?;

    let existing_market = MARKETS.load(deps.storage, market_index);
    if existing_market.is_ok() {
        return Err(ContractError::MarketIndexAlreadyInitialized {});
    }
    if amm_base_asset_reserve != amm_quote_asset_reserve {
        return Err(ContractError::InvalidInitialPeg.into());
    }

    let init_mark_price = helpers::amm::calculate_price(
        amm_quote_asset_reserve,
        amm_base_asset_reserve,
        amm_peg_multiplier,
    )?;

    let a = Amm {
        oracle: state.oracle,
        oracle_source,
        base_asset_reserve: amm_base_asset_reserve,
        quote_asset_reserve: amm_quote_asset_reserve,
        cumulative_repeg_rebate_long: Uint128::zero(),
        cumulative_repeg_rebate_short: Uint128::zero(),
        cumulative_funding_rate_long: Number128::zero(),
        cumulative_funding_rate_short: Number128::zero(),
        last_funding_rate: Number128::zero(),
        last_funding_rate_ts: now,
        funding_period: amm_periodicity,
        last_oracle_price_twap: Number128::zero(),
        last_mark_price_twap: init_mark_price,
        last_mark_price_twap_ts: now,
        sqrt_k: amm_base_asset_reserve,
        peg_multiplier: amm_peg_multiplier,
        total_fee: Uint128::zero(),
        total_fee_minus_distributions: Uint128::zero(),
        total_fee_withdrawn: Uint128::zero(),
        minimum_quote_asset_trade_size: Uint128::from(10000000 as u128),
        last_oracle_price_twap_ts: now,
        last_oracle_price: Number128::zero(),
        minimum_base_asset_trade_size: Uint128::from(10000000 as u128),
    };

    // Verify there's no overflow
    let _k = amm_base_asset_reserve.checked_mul(amm_quote_asset_reserve)?;

    let OraclePriceData {
        // price: oracle_price,
        ..
    } = a.get_oracle_price()?;

    // let last_oracle_price_twap = a.get_oracle_twap()?;

    helpers::margin_validation::validate_margin(
        margin_ratio_initial,
        margin_ratio_partial,
        margin_ratio_maintenance,
    )?;
    let market = Market {
        market_name: market_name,
        initialized: true,
        base_asset_amount_long: Number128::zero(),
        base_asset_amount_short: Number128::zero(),
        base_asset_amount: Number128::zero(),
        open_interest: Uint128::zero(),
        margin_ratio_initial, // unit is 20% (+2 decimal places)
        margin_ratio_partial,
        margin_ratio_maintenance,
        amm: a,
    };
    MARKETS.save(deps.storage, market_index, &market)?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.markets_length += 1;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "try_initialize_market"))
}

pub fn try_deposit_collateral(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: u64,
    referrer: Option<String>,
) -> Result<Response, ContractError> {
    let user_address = info.sender.clone();
    let existing_user = USERS.may_load(deps.storage, &user_address)?;
    let now = env.block.time.seconds();
    let mut user: User;
    if existing_user.is_some(){
        // user = existing_user.unwrap();
        user = User {
            collateral: Uint128::zero(),
            cumulative_deposits: Uint128::zero(),
            total_fee_paid: Uint128::zero(),
            total_token_discount: Uint128::zero(),
            total_referral_reward: Uint128::zero(),
            total_referee_discount: Uint128::zero(),
            positions_length: 0,
            referrer: None,
        };
    }
    else {
        if referrer.is_some() {
            user = User {
                collateral: Uint128::zero(),
                cumulative_deposits: Uint128::zero(),
                total_fee_paid: Uint128::zero(),
                total_token_discount: Uint128::zero(),
                total_referral_reward: Uint128::zero(),
                total_referee_discount: Uint128::zero(),
                positions_length: 0,
                referrer: Some(addr_validate_to_lower(deps.api, &referrer.unwrap())?),
            };
        } else {
            user = User {
                collateral: Uint128::zero(),
                cumulative_deposits: Uint128::zero(),
                total_fee_paid: Uint128::zero(),
                total_token_discount: Uint128::zero(),
                total_referral_reward: Uint128::zero(),
                total_referee_discount: Uint128::zero(),
                positions_length: 0,
                referrer: None,
            };
        }
    } 

    if amount == 0 {
        return Err(ContractError::InsufficientDeposit.into());
    }

    assert_sent_uusd_balance(&info.clone(), amount as u128)?;

    let collateral_before = user.collateral;
    let cumulative_deposits_before = user.cumulative_deposits;

    user.collateral = user.collateral.checked_add(Uint128::from(amount as u128))?;
    user.cumulative_deposits = user.cumulative_deposits.checked_add(amount.into())?;

    controller::funding::settle_funding_payment(&mut deps, &user_address, now)?;
    //get and send tokens to collateral vault
    let state = STATE.load(deps.storage)?;
    let message: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.insurance_vault.to_string(),
        msg: to_binary(&VaultInterface::Deposit {})?,
        funds: coins(amount.into(), "uusd"),
    });
    let deposit_history_info_length = DEPOSIT_HISTORY_INFO
        .load(deps.storage)?
        .len
        .checked_add(1)
        .ok_or_else(|| (ContractError::MathError))?;
    DEPOSIT_HISTORY_INFO.update(
        deps.storage,
        |mut i| -> Result<DepositInfo, ContractError> {
            i.len = deposit_history_info_length;
            Ok(i)
        },
    )?;
    DEPOSIT_HISTORY.save(
        deps.storage,
        (user_address.clone(), deposit_history_info_length as u64),
        &DepositRecord {
            ts: now,
            record_id: deposit_history_info_length,
            user: user_address.clone(),
            direction: DepositDirection::DEPOSIT,
            collateral_before,
            cumulative_deposits_before,
            amount: amount,
        },
    )?;
    if state.max_deposit.u128() > 0 && user.cumulative_deposits.u128() > state.max_deposit.u128() {
        return Err(ContractError::UserMaxDeposit.into());
    }
    USERS.update(
        deps.storage,
        &user_address.clone(),
        |_m| -> Result<User, ContractError> { Ok(user) },
    )?;
    Ok(Response::new()
        .add_message(message)
        .add_attribute("method", "try_deposit_collateral"))
}

pub fn try_withdraw_collateral(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: u64,
) -> Result<Response, ContractError> {
    let user_address = info.sender.clone();
    let existing_user = USERS.may_load(deps.storage, &user_address)?;
    let now = env.block.time.seconds();
    let mut user;
    if existing_user.is_none(){
        return Err(ContractError::UserDoesNotExist);
    }
    else{
        user = existing_user.unwrap();
    }
    let collateral_before = user.collateral;
    let cumulative_deposits_before = user.cumulative_deposits;

    controller::funding::settle_funding_payment(&mut deps, &user_address, now)?;

    if (amount as u128) > user.collateral.u128() {
        return Err(ContractError::InsufficientCollateral.into());
    }

    let state = STATE.load(deps.storage)?;
    let collateral_balance = query_balance(&deps.querier, state.collateral_vault.clone())?;
    let insurance_balance = query_balance(&deps.querier, state.insurance_vault.clone())?;
    let (collateral_account_withdrawal, insurance_account_withdrawal) =
        calculate_withdrawal_amounts(
            Uint128::from(amount as u128),
            Uint128::from(collateral_balance),
            Uint128::from(insurance_balance),
        )?;

    // amount_withdrawn can be less than amount if there is an insufficient balance in collateral and insurance vault
    let amount_withdraw =
        collateral_account_withdrawal.checked_add(insurance_account_withdrawal)?;

    user.cumulative_deposits = user
        .cumulative_deposits
        .checked_sub(Uint128::from(amount_withdraw))?;

    user.collateral = user
        .collateral
        .checked_sub(Uint128::from(collateral_account_withdrawal))?
        .checked_sub(Uint128::from(insurance_account_withdrawal))?;

    if !controller::margin::meets_initial_margin_requirement(&mut deps, &info.sender.clone())? {
        return Err(ContractError::InsufficientCollateral.into());
    }

    let mut messages: Vec<CosmosMsg> = vec![];

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.collateral_vault.clone().to_string(),
        msg: to_binary(&VaultInterface::Withdraw {
            to_address: info.sender.clone(),
            amount: collateral_account_withdrawal.u128(),
        })?,
        funds: vec![],
    }));

    if insurance_account_withdrawal.gt(&Uint128::zero()) {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.insurance_vault.to_string(),
            msg: to_binary(&VaultInterface::Withdraw {
                to_address: info.sender.clone(),
                amount: insurance_account_withdrawal.u128(),
            })?,
            funds: vec![],
        }));
    }

    let deposit_history_info_length = DEPOSIT_HISTORY_INFO
        .load(deps.storage)?
        .len
        .checked_add(1)
        .ok_or_else(|| (ContractError::MathError))?;
    DEPOSIT_HISTORY_INFO.update(
        deps.storage,
        |mut i| -> Result<DepositInfo, ContractError> {
            i.len = deposit_history_info_length;
            Ok(i)
        },
    )?;
    DEPOSIT_HISTORY.save(
        deps.storage,
        (user_address.clone(), deposit_history_info_length as u64),
        &DepositRecord {
            ts: now,
            record_id: deposit_history_info_length,
            user: user_address.clone(),
            direction: DepositDirection::WITHDRAW,
            collateral_before,
            cumulative_deposits_before,
            amount: amount_withdraw.u128() as u64,
        },
    )?;
    USERS.update(
        deps.storage,
        &user_address.clone(),
        |_u| -> Result<User, ContractError> { Ok(user) },
    )?;
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "try_withdraw_collateral"))
}

pub fn try_open_position(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    direction: PositionDirection,
    quote_asset_amount: Uint128,
    market_index: u64,
    limit_price: Uint128,
) -> Result<Response, ContractError> {
    let user_address = info.sender.clone();
    let mut user = USERS.load(deps.storage, &user_address)?;
    let now = env.block.time.seconds();
    let state = STATE.load(deps.storage)?;
    let oracle_guard_rails = ORACLEGUARDRAILS.load(deps.storage)?;
    let fee_structure = FEESTRUCTURE.load(deps.storage)?;

    if quote_asset_amount.is_zero() {
        return Err(ContractError::TradeSizeTooSmall.into());
    }
    controller::funding::settle_funding_payment(&mut deps, &user_address, now)?;

    let position_index = market_index.clone();
    let mark_price_before: Uint128;
    let oracle_mark_spread_pct_before: i128;
    let is_oracle_valid: bool;

    {
        let market = MARKETS.load(deps.storage, market_index)?;
        mark_price_before = market.amm.mark_price()?;
        let oracle_price_data = &market.amm.get_oracle_price()?;
        oracle_mark_spread_pct_before = helpers::amm::calculate_oracle_mark_spread_pct(
            &market.amm,
            oracle_price_data,
            Some(mark_price_before),
        )?;
        is_oracle_valid =
            helpers::amm::is_oracle_valid(&market.amm, oracle_price_data, &oracle_guard_rails)?;
        if is_oracle_valid {
            let normalised_oracle_price = helpers::amm::normalise_oracle_price(
                &market.amm,
                oracle_price_data,
                Some(mark_price_before),
            )?;
            controller::amm::update_oracle_price_twap(
                &mut deps,
                market_index,
                now,
                normalised_oracle_price,
            )?;
        }
    }

    let potentially_risk_increasing;
    let base_asset_amount;
    let mut quote_asset_amount = quote_asset_amount;

    {
        let (_potentially_risk_increasing, _, _base_asset_amount, _quote_asset_amount, _) =
            controller::position::update_position_with_quote_asset_amount(
                &mut deps,
                quote_asset_amount,
                direction,
                &user_address,
                position_index,
                mark_price_before,
                now,
            )?;

        potentially_risk_increasing = _potentially_risk_increasing;
        base_asset_amount = _base_asset_amount;
        quote_asset_amount = _quote_asset_amount;
    }

    let mark_price_after: Uint128;
    let oracle_price_after: i128;
    let oracle_mark_spread_pct_after: i128;
    {
        let market = MARKETS.load(deps.storage, market_index)?;
        mark_price_after = market.amm.mark_price()?;
        let oracle_price_data = market.amm.get_oracle_price()?;
        oracle_mark_spread_pct_after = helpers::amm::calculate_oracle_mark_spread_pct(
            &market.amm,
            &oracle_price_data,
            Some(mark_price_after),
        )?;
        oracle_price_after = oracle_price_data.price.i128();
    }

    let meets_initial_margin_requirement =
        controller::margin::meets_initial_margin_requirement(&mut deps, &user_address)?;
    if !meets_initial_margin_requirement && potentially_risk_increasing {
        return Err(ContractError::InsufficientCollateral.into());
    }

    // todo add referrer and discount token
    let referrer = user.referrer.clone();
    let discount_token = Uint128::zero();
    let (user_fee, fee_to_market, token_discount, referrer_reward, referee_discount) =
        helpers::fees::calculate_fee_for_trade(
            quote_asset_amount,
            &fee_structure,
            discount_token,
            &referrer,
        )?;

    {
        let mut market = MARKETS.load(deps.storage, market_index)?;
        market.amm.total_fee = market.amm.total_fee.checked_add(fee_to_market)?;
        market.amm.total_fee_minus_distributions = market
            .amm
            .total_fee_minus_distributions
            .checked_add(fee_to_market)?;
        MARKETS.update(
            deps.storage,
            market_index,
            |_m| -> Result<Market, ContractError> { Ok(market) },
        )?;
    }

    if user.collateral.ge(&user_fee) {
        user.collateral = user.collateral.checked_sub(user_fee)?;
    } else {
        user.collateral = Uint128::zero();
    }

    // Increment the user's total fee variables
    user.total_fee_paid = user.total_fee_paid.checked_add(user_fee)?;
    user.total_token_discount = user.total_token_discount.checked_add(token_discount)?;
    user.total_referee_discount = user.total_referee_discount.checked_add(referee_discount)?;

    // Update the referrer's collateral with their reward
    if referrer.is_some() {
        let mut _referrer = USERS.load(deps.storage, &referrer.clone().unwrap())?;
        _referrer.total_referral_reward = _referrer
            .total_referral_reward
            .checked_add(referrer_reward)?;
        // todo what this signifies
        // referrer.exit(ctx.program_id)?;
        USERS.update(
            deps.storage,
            &referrer.unwrap().clone(),
            |_m| -> Result<User, ContractError> { Ok(_referrer) },
        )?;
    }

    let is_oracle_mark_too_divergent_before = helpers::amm::is_oracle_mark_too_divergent(
        oracle_mark_spread_pct_before,
        &oracle_guard_rails,
    )?;
    let is_oracle_mark_too_divergent_after = helpers::amm::is_oracle_mark_too_divergent(
        oracle_mark_spread_pct_after,
        &oracle_guard_rails,
    )?;

    if is_oracle_mark_too_divergent_after && !is_oracle_mark_too_divergent_before && is_oracle_valid
    {
        return Err(ContractError::OracleMarkSpreadLimit.into());
    }
    let trade_history_info_length = TRADE_HISTORY_INFO
        .load(deps.storage)?
        .len
        .checked_add(1)
        .ok_or_else(|| (ContractError::MathError))?;
    TRADE_HISTORY_INFO.update(deps.storage, |mut i| -> Result<TradeInfo, ContractError> {
        i.len = trade_history_info_length;
        Ok(i)
    })?;

    TRADE_HISTORY.save(
        deps.storage,
        trade_history_info_length,
        &TradeRecord {
            ts: now,
            user: user_address.clone(),
            direction,
            base_asset_amount,
            quote_asset_amount,
            mark_price_before,
            mark_price_after,
            fee: user_fee,
            referrer_reward,
            referee_discount,
            token_discount,
            liquidation: false,
            market_index,
            oracle_price: Number128::new(oracle_price_after),
        },
    )?;

    if !limit_price.is_zero()
        && !helpers::order::limit_price_satisfied(
            limit_price,
            quote_asset_amount,
            base_asset_amount,
            direction,
        )?
    {
        return Err(ContractError::SlippageOutsideLimit.into());
    }

    {
        controller::funding::update_funding_rate(
            &mut deps,
            market_index,
            now,
            state.funding_paused,
            Some(mark_price_before),
        )?;
    }

    USERS.update(
        deps.storage,
        &user_address.clone(),
        |_m| -> Result<User, ContractError> { Ok(user) },
    )?;

    Ok(Response::new().add_attribute("method", "try_open_position"))
}

pub fn try_close_position(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    market_index: u64,
) -> Result<Response, ContractError> {
    let user_address = info.sender.clone();
    let mut user = USERS.load(deps.storage, &user_address)?;
    let now = env.block.time.seconds();
    let state = STATE.load(deps.storage)?;
    let oracle_guard_rails = ORACLEGUARDRAILS.load(deps.storage)?;
    let fee_structure = FEESTRUCTURE.load(deps.storage)?;
    controller::funding::settle_funding_payment(&mut deps, &user_address, now)?;

    let position_index = market_index.clone();
    let market_position = POSITIONS.load(deps.storage, (&user_address.clone(), market_index))?;
    let mut market = MARKETS.load(deps.storage, market_index)?;
    let mark_price_before = market.amm.mark_price()?;
    let oracle_price_data = market.amm.get_oracle_price()?;
    let oracle_mark_spread_pct_before = helpers::amm::calculate_oracle_mark_spread_pct(
        &market.amm,
        &oracle_price_data,
        Some(mark_price_before),
    )?;
    let direction_to_close =
        helpers::position::direction_to_close_position(market_position.base_asset_amount.i128());

    let (quote_asset_amount, base_asset_amount, _) = controller::position::close(
        &mut deps,
        &user_address,
        market_index,
        position_index,
        now,
        None,
        Some(mark_price_before),
    )?;
    let base_asset_amount = Uint128::from(base_asset_amount.unsigned_abs());
    let referrer = user.referrer.clone();
    let discount_token = Uint128::zero();

    let (user_fee, fee_to_market, token_discount, referrer_reward, referee_discount) =
        helpers::fees::calculate_fee_for_trade(
            quote_asset_amount,
            &fee_structure,
            discount_token,
            &referrer,
        )?;

    market.amm.total_fee = market.amm.total_fee.checked_add(fee_to_market)?;
    market.amm.total_fee_minus_distributions = market
        .amm
        .total_fee_minus_distributions
        .checked_add(fee_to_market)?;

    if user.collateral.gt(&user_fee) {
        user.collateral = user.collateral.checked_sub(user_fee)?;
    } else {
        user.collateral = Uint128::zero();
    }

    user.total_fee_paid = user.total_fee_paid.checked_add(user_fee)?;
    user.total_token_discount = user.total_token_discount.checked_add(token_discount)?;
    user.total_referee_discount = user.total_referee_discount.checked_add(referee_discount)?;

    if referrer.is_some() {
        let mut _referrer = USERS.load(deps.storage, &referrer.clone().unwrap())?;
        _referrer.total_referral_reward = _referrer
            .total_referral_reward
            .checked_add(referrer_reward)?;
        USERS.update(
            deps.storage,
            &referrer.unwrap().clone(),
            |_m| -> Result<User, ContractError> { Ok(_referrer) },
        )?;
    }

    let mark_price_after = market.amm.mark_price()?;

    let oracle_mark_spread_pct_after = helpers::amm::calculate_oracle_mark_spread_pct(
        &market.amm,
        &oracle_price_data,
        Some(mark_price_after),
    )?;

    let oracle_price_after = oracle_price_data.price;

    let is_oracle_valid =
        helpers::amm::is_oracle_valid(&market.amm, &oracle_price_data, &oracle_guard_rails)?;
    if is_oracle_valid {
        let normalised_oracle_price = helpers::amm::normalise_oracle_price(
            &market.amm,
            &oracle_price_data,
            Some(mark_price_before),
        )?;
        controller::amm::update_oracle_price_twap(
            &mut deps,
            market_index,
            now,
            normalised_oracle_price,
        )?;
    }

    let is_oracle_mark_too_divergent_before = helpers::amm::is_oracle_mark_too_divergent(
        oracle_mark_spread_pct_before,
        &oracle_guard_rails,
    )?;
    let is_oracle_mark_too_divergent_after = helpers::amm::is_oracle_mark_too_divergent(
        oracle_mark_spread_pct_after,
        &oracle_guard_rails,
    )?;

    if (is_oracle_mark_too_divergent_after && !is_oracle_mark_too_divergent_before)
        && is_oracle_valid
    {
        return Err(ContractError::OracleMarkSpreadLimit.into());
    }

    let trade_history_info_length = TRADE_HISTORY_INFO
        .load(deps.storage)?
        .len
        .checked_add(1)
        .ok_or_else(|| (ContractError::MathError))?;
    TRADE_HISTORY_INFO.update(deps.storage, |mut i| -> Result<TradeInfo, ContractError> {
        i.len = trade_history_info_length;
        Ok(i)
    })?;

    TRADE_HISTORY.save(
        deps.storage,
        trade_history_info_length,
        &TradeRecord {
            ts: now,
            user: user_address.clone(),
            direction: direction_to_close,
            base_asset_amount,
            quote_asset_amount,
            mark_price_before,
            mark_price_after,
            fee: user_fee,
            referrer_reward,
            referee_discount,
            token_discount,
            liquidation: false,
            market_index,
            oracle_price: oracle_price_after,
        },
    )?;

    controller::funding::update_funding_rate(
        &mut deps,
        market_index,
        now,
        state.funding_paused,
        Some(mark_price_before),
    )?;
    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> { Ok(market) },
    )?;

    USERS.update(
        deps.storage,
        &user_address.clone(),
        |_m| -> Result<User, ContractError> { Ok(user) },
    )?;

    Ok(Response::new().add_attribute("method", "try_close_position"))
}

//new limit order interfaces
pub fn try_place_order(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    order: OrderParams,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let user_address = info.sender.clone();
    let state = STATE.load(deps.storage)?;
    let oracle = state.oracle;
    if order.order_type == OrderType::Market {
        return Err(ContractError::MarketOrderMustBeInPlaceAndFill.into());
    }

    controller::order::place_order(&mut deps, &user_address, now, order, &oracle)?;
    Ok(Response::new().add_attribute("method", "try_place_order"))
}

pub fn try_cancel_order(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    market_index: u64,
    order_id: u64,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let state = STATE.load(deps.storage)?;
    let oracle = state.oracle;
    controller::order::cancel_order(
        &mut deps,
        &info.sender.clone(),
        market_index,
        order_id,
        &oracle,
        now,
    )?;
    Ok(Response::new().add_attribute("method", "try_cancel_order"))
}

//todo who is filler? is sender is filler and passing the user address?
pub fn try_expire_orders(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user_address: String,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let user_address = addr_validate_to_lower(deps.api, &user_address.to_string())?;
    controller::order::expire_orders(&mut deps, &user_address, now, &info.sender.clone())?;
    Ok(Response::new().add_attribute("method", "try_expire_orders"))
}

pub fn try_fill_order(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    order_id: u64,
    user_address: String,
    market_index: u64,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let user_address = addr_validate_to_lower(deps.api, &user_address.to_string())?;
    let base_asset_amount = controller::order::fill_order(
        &mut deps,
        &user_address,
        &info.sender.clone(),
        market_index,
        order_id,
        now,
    )?;
    if base_asset_amount.is_zero() {
        return Err(ContractError::CouldNotFillOrder);
    }
    Ok(Response::new().add_attribute("method", "try_fill_order"))
}

//todo later

pub fn try_liquidate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user: String,
    market_index: u64,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;
    let oracle_guard_rails = ORACLEGUARDRAILS.load(deps.storage)?;
    let user_address = addr_validate_to_lower(deps.api, &user)?;
    let now = env.block.time.seconds();
    let mut user = USERS.load(deps.storage, &user_address)?;
    // let user_position = Positions.load(deps.storage, (&user_address, market_index))?;

    controller::funding::settle_funding_payment(&mut deps, &user_address, now)?;

    let LiquidationStatus {
        liquidation_type,
        total_collateral,
        adjusted_total_collateral,
        unrealized_pnl,
        base_asset_value,
        market_statuses,
        mut margin_requirement,
        margin_ratio,
    } = controller::margin::calculate_liquidation_status(
        &mut deps,
        &user_address,
        &oracle_guard_rails,
    )?;

    let res: Response = Response::new().add_attribute("method", "try_liquidate");
    let collateral = user.collateral;
    if liquidation_type == LiquidationType::NONE {
        res.clone()
            .add_attribute("total_collateral {}", total_collateral.to_string());
        res.clone().add_attribute(
            "adjusted_total_collateral {}",
            adjusted_total_collateral.to_string(),
        );
        res.clone()
            .add_attribute("margin_requirement {}", margin_requirement.to_string());
        return Err(ContractError::SufficientCollateral.into());
    }

    let is_dust_position = adjusted_total_collateral <= QUOTE_PRECISION;

    let mut base_asset_value_closed: Uint128 = Uint128::zero();
    let mut liquidation_fee = Uint128::zero();

    let is_full_liquidation = liquidation_type == LiquidationType::FULL || is_dust_position;

    if is_full_liquidation {
        let maximum_liquidation_fee = total_collateral
            .checked_mul(state.full_liquidation_penalty_percentage.numerator())?
            .checked_div(state.full_liquidation_penalty_percentage.denominator())?;

        for market_status in market_statuses.iter() {
            if market_status.base_asset_value.is_zero() {
                continue;
            }

            let market = MARKETS.load(deps.storage, market_status.market_index)?;
            let mark_price_before = market_status.mark_price_before;
            let oracle_status = &market_status.oracle_status;

            // if the oracle is invalid and the mark moves too far from twap, dont liquidate
            let oracle_is_valid = oracle_status.is_valid;
            if !oracle_is_valid {
                let mark_twap_divergence =
                    helpers::amm::calculate_mark_twap_spread_pct(&market.amm, mark_price_before)?;
                let mark_twap_too_divergent =
                    mark_twap_divergence.unsigned_abs() >= MAX_MARK_TWAP_DIVERGENCE.u128();

                if mark_twap_too_divergent {
                    res.clone().add_attribute(
                        "mark_twap_divergence {} for market {}",
                        mark_twap_divergence.to_string(),
                    );
                    continue;
                }
            }

            let market_position = POSITIONS.load(deps.storage, (&user_address, market_index))?;
            // todo initialize position

            let mark_price_before_i128 = mark_price_before.u128() as i128;
            let close_position_slippage = match market_status.close_position_slippage {
                Some(close_position_slippage) => close_position_slippage,
                None => helpers::slippage::calculate_slippage(
                    market_status.base_asset_value,
                    Uint128::from(market_position.base_asset_amount.i128().unsigned_abs()),
                    mark_price_before_i128,
                )?,
            };
            let close_position_slippage_pct = helpers::slippage::calculate_slippage_pct(
                close_position_slippage,
                mark_price_before_i128,
            )?;

            let close_slippage_pct_too_large = close_position_slippage_pct
                > MAX_LIQUIDATION_SLIPPAGE.u128() as i128
                || close_position_slippage_pct < -(MAX_LIQUIDATION_SLIPPAGE.u128() as i128);

            let oracle_mark_divergence_after_close = if !close_slippage_pct_too_large {
                oracle_status
                    .oracle_mark_spread_pct
                    .i128()
                    .checked_add(close_position_slippage_pct)
                    .ok_or_else(|| (ContractError::MathError))?
            } else if close_position_slippage_pct > 0 {
                oracle_status
                    .oracle_mark_spread_pct
                    .i128()
                    // approximates price impact based on slippage
                    .checked_add((MAX_LIQUIDATION_SLIPPAGE.u128() as i128) * 2)
                    .ok_or_else(|| (ContractError::MathError))?
            } else {
                oracle_status
                    .oracle_mark_spread_pct
                    .i128()
                    // approximates price impact based on slippage
                    .checked_sub((MAX_LIQUIDATION_SLIPPAGE.u128() as i128) * 2)
                    .ok_or_else(|| (ContractError::MathError))?
            };

            let oracle_mark_too_divergent_after_close = helpers::amm::is_oracle_mark_too_divergent(
                oracle_mark_divergence_after_close,
                &oracle_guard_rails,
            )?;

            // if closing pushes outside the oracle mark threshold, don't liquidate
            if oracle_is_valid && oracle_mark_too_divergent_after_close {
                // but only skip the liquidation if it makes the divergence worse
                if oracle_status.oracle_mark_spread_pct.i128().unsigned_abs()
                    < oracle_mark_divergence_after_close.unsigned_abs()
                {
                    res.clone().add_attribute(
                        "oracle_mark_divergence_after_close ",
                        oracle_mark_divergence_after_close.to_string(),
                    );
                    continue;
                }
            }

            let direction_to_close = helpers::position::direction_to_close_position(
                market_position.base_asset_amount.i128(),
            );

            // just reduce position if position is too big
            let (quote_asset_amount, base_asset_amount) = if close_slippage_pct_too_large {
                let quote_asset_amount = market_status
                    .base_asset_value
                    .checked_mul(MAX_LIQUIDATION_SLIPPAGE)?
                    .checked_div(Uint128::from(close_position_slippage_pct.unsigned_abs()))?;

                let base_asset_amount = controller::position::reduce(
                    &mut deps,
                    direction_to_close,
                    quote_asset_amount,
                    &user_address,
                    market_index,
                    market_index,
                    now,
                    Some(mark_price_before),
                )?;

                (quote_asset_amount, base_asset_amount)
            } else {
                let (quote_asset_amount, base_asset_amount, _) = controller::position::close(
                    &mut deps,
                    &user_address,
                    market_index,
                    market_index,
                    now,
                    None,
                    Some(mark_price_before),
                )?;

                (quote_asset_amount, base_asset_amount)
            };

            let base_asset_amount = Uint128::from(base_asset_amount.unsigned_abs());
            base_asset_value_closed = base_asset_value_closed.checked_add(quote_asset_amount)?;
            let mark_price_after = market.amm.mark_price()?;

            let trade_history_info_length = TRADE_HISTORY_INFO
                .load(deps.storage)?
                .len
                .checked_add(1)
                .ok_or_else(|| (ContractError::MathError))?;
            TRADE_HISTORY_INFO.update(
                deps.storage,
                |mut i| -> Result<TradeInfo, ContractError> {
                    i.len = trade_history_info_length;
                    Ok(i)
                },
            )?;

            TRADE_HISTORY.save(
                deps.storage,
                trade_history_info_length,
                &TradeRecord {
                    ts: now,
                    user: user_address.clone(),
                    direction: direction_to_close,
                    base_asset_amount,
                    quote_asset_amount,
                    mark_price_before,
                    mark_price_after,
                    fee: Uint128::zero(),
                    referrer_reward: Uint128::zero(),
                    referee_discount: Uint128::zero(),
                    token_discount: Uint128::zero(),
                    liquidation: true,
                    market_index,
                    oracle_price: market_status.oracle_status.price_data.price,
                },
            )?;

            margin_requirement = margin_requirement.checked_sub(
                market_status
                    .maintenance_margin_requirement
                    .checked_mul(quote_asset_amount)?
                    .checked_div(market_status.base_asset_value)?,
            )?;

            let market_liquidation_fee = maximum_liquidation_fee
                .checked_mul(quote_asset_amount)?
                .checked_div(base_asset_value)?;

            liquidation_fee = liquidation_fee.checked_add(market_liquidation_fee)?;

            let adjusted_total_collateral_after_fee =
                adjusted_total_collateral.checked_sub(liquidation_fee)?;

            if !is_dust_position && margin_requirement < adjusted_total_collateral_after_fee {
                break;
            }
        }
    } else {
        let maximum_liquidation_fee = total_collateral
            .checked_mul(state.partial_liquidation_penalty_percentage.numerator())?
            .checked_div(state.partial_liquidation_penalty_percentage.denominator())?;
        let maximum_base_asset_value_closed = base_asset_value
            .checked_mul(state.partial_liquidation_close_percentage.numerator())?
            .checked_div(state.partial_liquidation_close_percentage.denominator())?;
        for market_status in market_statuses.iter() {
            if market_status.base_asset_value.is_zero() {
                continue;
            }

            let oracle_status = &market_status.oracle_status;
            let market = MARKETS.load(deps.storage, market_index)?;
            let mark_price_before = market_status.mark_price_before;

            let oracle_is_valid = oracle_status.is_valid;
            if !oracle_is_valid {
                let mark_twap_divergence =
                    helpers::amm::calculate_mark_twap_spread_pct(&market.amm, mark_price_before)?;
                let mark_twap_too_divergent =
                    mark_twap_divergence.unsigned_abs() >= MAX_MARK_TWAP_DIVERGENCE.u128();

                if mark_twap_too_divergent {
                    res.clone()
                        .add_attribute("mark_twap_divergence", mark_twap_divergence.to_string());
                    continue;
                }
            }

            let market_position = POSITIONS.load(deps.storage, (&user_address, market_index))?;

            let mut quote_asset_amount = market_status
                .base_asset_value
                .checked_mul(state.partial_liquidation_close_percentage.numerator())?
                .checked_div(state.partial_liquidation_close_percentage.denominator())?;

            let mark_price_before_i128 = mark_price_before.u128() as i128;
            let reduce_position_slippage = match market_status.close_position_slippage {
                Some(close_position_slippage) => close_position_slippage.div(4),
                None => helpers::slippage::calculate_slippage(
                    market_status.base_asset_value,
                    Uint128::from(market_position.base_asset_amount.i128().unsigned_abs()),
                    mark_price_before_i128,
                )?
                .div(4),
            };

            let reduce_position_slippage_pct = helpers::slippage::calculate_slippage_pct(
                reduce_position_slippage,
                mark_price_before_i128,
            )?;

            res.clone().add_attribute(
                "reduce_position_slippage_pct",
                reduce_position_slippage_pct.to_string(),
            );

            let reduce_slippage_pct_too_large = reduce_position_slippage_pct
                > (MAX_LIQUIDATION_SLIPPAGE.u128() as i128)
                || reduce_position_slippage_pct < -(MAX_LIQUIDATION_SLIPPAGE.u128() as i128);

            let oracle_mark_divergence_after_reduce = if !reduce_slippage_pct_too_large {
                oracle_status
                    .oracle_mark_spread_pct
                    .i128()
                    .checked_add(reduce_position_slippage_pct)
                    .ok_or_else(|| (ContractError::MathError))?
            } else if reduce_position_slippage_pct > 0 {
                oracle_status
                    .oracle_mark_spread_pct
                    .i128()
                    // approximates price impact based on slippage
                    .checked_add((MAX_LIQUIDATION_SLIPPAGE.u128() as i128) * 2)
                    .ok_or_else(|| (ContractError::MathError))?
            } else {
                oracle_status
                    .oracle_mark_spread_pct
                    .i128()
                    // approximates price impact based on slippage
                    .checked_sub((MAX_LIQUIDATION_SLIPPAGE.u128() as i128) * 2)
                    .ok_or_else(|| (ContractError::MathError))?
            };

            let oracle_mark_too_divergent_after_reduce =
                helpers::amm::is_oracle_mark_too_divergent(
                    oracle_mark_divergence_after_reduce,
                    &oracle_guard_rails,
                )?;

            // if reducing pushes outside the oracle mark threshold, don't liquidate
            if oracle_is_valid && oracle_mark_too_divergent_after_reduce {
                // but only skip the liquidation if it makes the divergence worse
                if oracle_status.oracle_mark_spread_pct.i128().unsigned_abs()
                    < oracle_mark_divergence_after_reduce.unsigned_abs()
                {
                    res.clone().add_attribute(
                        "oracle_mark_spread_pct_after_reduce",
                        oracle_mark_divergence_after_reduce.to_string(),
                    );
                    return Err(ContractError::OracleMarkSpreadLimit.into());
                }
            }

            if reduce_slippage_pct_too_large {
                quote_asset_amount = quote_asset_amount
                    .checked_mul(MAX_LIQUIDATION_SLIPPAGE)?
                    .checked_div(Uint128::from(reduce_position_slippage_pct.unsigned_abs()))?;
            }

            base_asset_value_closed = base_asset_value_closed.checked_add(quote_asset_amount)?;

            let direction_to_reduce = helpers::position::direction_to_close_position(
                market_position.base_asset_amount.i128(),
            );

            let base_asset_amount = controller::position::reduce(
                &mut deps,
                direction_to_reduce,
                quote_asset_amount,
                &user_address,
                market_index,
                market_index,
                now,
                Some(mark_price_before),
            )?
            .unsigned_abs();

            let mark_price_after = market.amm.mark_price()?;

            let trade_history_info_length = TRADE_HISTORY_INFO
                .load(deps.storage)?
                .len
                .checked_add(1)
                .ok_or_else(|| (ContractError::MathError))?;
            TRADE_HISTORY_INFO.update(
                deps.storage,
                |mut i| -> Result<TradeInfo, ContractError> {
                    i.len = trade_history_info_length;
                    Ok(i)
                },
            )?;

            TRADE_HISTORY.save(
                deps.storage,
                trade_history_info_length,
                &TradeRecord {
                    ts: now,
                    user: user_address.clone(),
                    direction: direction_to_reduce,
                    base_asset_amount: Uint128::from(base_asset_amount),
                    quote_asset_amount,
                    mark_price_before,
                    mark_price_after,
                    fee: Uint128::zero(),
                    referrer_reward: Uint128::zero(),
                    referee_discount: Uint128::zero(),
                    token_discount: Uint128::zero(),
                    liquidation: true,
                    market_index,
                    oracle_price: market_status.oracle_status.price_data.price,
                },
            )?;

            margin_requirement = margin_requirement.checked_sub(
                market_status
                    .partial_margin_requirement
                    .checked_mul(quote_asset_amount)?
                    .checked_div(market_status.base_asset_value)?,
            )?;

            let market_liquidation_fee = maximum_liquidation_fee
                .checked_mul(quote_asset_amount)?
                .checked_div(maximum_base_asset_value_closed)?;

            liquidation_fee = liquidation_fee.checked_add(market_liquidation_fee)?;

            let adjusted_total_collateral_after_fee =
                adjusted_total_collateral.checked_sub(liquidation_fee)?;

            if margin_requirement < adjusted_total_collateral_after_fee {
                break;
            }
        }
    }
    if base_asset_value_closed.is_zero() {
        return Err(ContractError::NoPositionsLiquidatable);
    }

    let balance_collateral = query_balance(&deps.querier, state.collateral_vault.clone())?;

    let balance_insurance = query_balance(&deps.querier, state.insurance_vault.clone())?;

    let (withdrawal_amount, _) = calculate_withdrawal_amounts(
        liquidation_fee,
        Uint128::from(balance_collateral),
        Uint128::from(balance_insurance),
    )?;

    user.collateral = user.collateral.checked_sub(liquidation_fee)?;

    let fee_to_liquidator = if is_full_liquidation {
        withdrawal_amount.checked_div(Uint128::from(
            state.full_liquidation_liquidator_share_denominator,
        ))?
    } else {
        withdrawal_amount.checked_div(Uint128::from(
            state.partial_liquidation_liquidator_share_denominator,
        ))?
    };

    let fee_to_insurance_fund = withdrawal_amount.checked_sub(fee_to_liquidator)?;

    if fee_to_liquidator.gt(&Uint128::zero()) {
        let mut liquidator = USERS.load(deps.storage, &info.sender.clone())?;
        liquidator.collateral = liquidator
            .collateral
            .checked_add(Uint128::from(fee_to_liquidator))?;

        USERS.update(
            deps.storage,
            &info.sender.clone(),
            |_m| -> Result<User, ContractError> { Ok(liquidator) },
        )?;
    }
    let mut messages: Vec<CosmosMsg> = vec![];
    if fee_to_insurance_fund.gt(&Uint128::zero()) {
        let message = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.collateral_vault.to_string(),
            msg: to_binary(&VaultInterface::Withdraw {
                to_address: state.insurance_vault.clone(),
                amount: fee_to_insurance_fund.u128(),
            })?,
            funds: vec![],
        });
        messages.push(message);
    }

    let liquidation_history_info_length = LIQUIDATION_HISTORY_INFO
        .load(deps.storage)?
        .len
        .checked_add(1)
        .ok_or_else(|| (ContractError::MathError))?;
    LIQUIDATION_HISTORY_INFO.update(
        deps.storage,
        |mut i| -> Result<LiquidationInfo, ContractError> {
            i.len = liquidation_history_info_length;
            Ok(i)
        },
    )?;
    LIQUIDATION_HISTORY.save(
        deps.storage,
        (user_address.clone(), liquidation_history_info_length as u64),
        &LiquidationRecord {
            ts: now,
            record_id: liquidation_history_info_length,
            user: user_address,
            partial: !is_full_liquidation,
            base_asset_value,
            base_asset_value_closed,
            liquidation_fee,
            liquidator: info.sender.clone(),
            total_collateral,
            collateral,
            unrealized_pnl: Number128::new(unrealized_pnl),
            margin_ratio,
            fee_to_liquidator: fee_to_liquidator.u128() as u64,
            fee_to_insurance_fund: fee_to_insurance_fund.u128() as u64,
        },
    )?;
    Ok(res.add_messages(messages))
}

pub fn try_move_amm_price(
    mut deps: DepsMut,
    base_asset_reserve: Uint128,
    quote_asset_reserve: Uint128,
    market_index: u64,
) -> Result<Response, ContractError> {
    controller::amm::move_price(
        &mut deps,
        market_index,
        base_asset_reserve,
        quote_asset_reserve,
    )?;
    Ok(Response::new().add_attribute("method", "try_move_amm_price"))
}

pub fn try_withdraw_fees(
    deps: DepsMut,
    info: MessageInfo,
    market_index: u64,
    amount: u64,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let state = STATE.load(deps.storage)?;
    let mut market = MARKETS.load(deps.storage, market_index)?;

    // A portion of fees must always remain in protocol to be used to keep markets optimal
    let max_withdraw = market
        .amm
        .total_fee
        .checked_mul(SHARE_OF_FEES_ALLOCATED_TO_CLEARING_HOUSE_NUMERATOR)?
        .checked_div(SHARE_OF_FEES_ALLOCATED_TO_CLEARING_HOUSE_DENOMINATOR)?
        .checked_sub(market.amm.total_fee_withdrawn)?;

    if amount as u128 > max_withdraw.u128() {
        return Err(ContractError::AdminWithdrawTooLarge.into());
    }

    //todo recipient who? is it only admin function
    let message: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.collateral_vault.to_string(),
        msg: to_binary(&VaultInterface::Withdraw {
            to_address: info.sender.clone(),
            amount: amount as u128,
        })?,
        funds: vec![],
    });

    market.amm.total_fee_withdrawn = market
        .amm
        .total_fee_withdrawn
        .checked_add(Uint128::from(amount))?;

    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> { Ok(market) },
    )?;

    Ok(Response::new()
        .add_message(message)
        .add_attribute("method", "try_withdraw_fees"))
}

pub fn try_withdraw_from_insurance_vault_to_market(
    deps: DepsMut,
    info: MessageInfo,
    market_index: u64,
    amount: u64,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let state = STATE.load(deps.storage)?;

    let mut market = MARKETS.load(deps.storage, market_index)?;
    market.amm.total_fee_minus_distributions = market
        .amm
        .total_fee_minus_distributions
        .checked_add(Uint128::from(amount))?;

    let message: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.insurance_vault.to_string(),
        msg: to_binary(&VaultInterface::Withdraw {
            to_address: state.collateral_vault.clone(),
            amount: amount as u128,
        })?,
        funds: vec![],
    });
    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> { Ok(market) },
    )?;
    Ok(Response::new()
        .add_message(message)
        .add_attribute("method", "try_withdraw_from_insurance_vault_to_market"))
}

pub fn try_repeg_amm_curve(
    mut deps: DepsMut,
    env: Env,
    new_peg_candidate: Uint128,
    market_index: u64,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let market = MARKETS.load(deps.storage, market_index)?;
    let OraclePriceData {
        price: oracle_price,
        ..
    } = market.amm.get_oracle_price()?;
    let peg_multiplier_before = market.amm.peg_multiplier;
    let base_asset_reserve_before = market.amm.base_asset_reserve;
    let quote_asset_reserve_before = market.amm.quote_asset_reserve;
    let sqrt_k_before = market.amm.sqrt_k;

    // let price_oracle = state.oracle;

    let adjustment_cost =
        controller::repeg::repeg(&mut deps, market_index, new_peg_candidate).unwrap();
    let peg_multiplier_after = market.amm.peg_multiplier;
    let base_asset_reserve_after = market.amm.base_asset_reserve;
    let quote_asset_reserve_after = market.amm.quote_asset_reserve;
    let sqrt_k_after = market.amm.sqrt_k;

    let curve_history_info_length = CURVE_HISTORY_INFO
        .load(deps.storage)?
        .len
        .checked_add(1)
        .ok_or_else(|| (ContractError::MathError))?;
    CURVE_HISTORY_INFO.update(
        deps.storage,
        |mut i: CurveInfo| -> Result<CurveInfo, ContractError> {
            i.len = curve_history_info_length;
            Ok(i)
        },
    )?;

    CURVEHISTORY.save(
        deps.storage,
        curve_history_info_length as u64,
        &CurveRecord {
            ts: now,
            record_id: curve_history_info_length,
            market_index,
            peg_multiplier_before,
            base_asset_reserve_before,
            quote_asset_reserve_before,
            sqrt_k_before,
            peg_multiplier_after,
            base_asset_reserve_after,
            quote_asset_reserve_after,
            sqrt_k_after,
            base_asset_amount_long: Uint128::from(
                market.base_asset_amount_long.i128().unsigned_abs(),
            ),
            base_asset_amount_short: Uint128::from(
                market.base_asset_amount_short.i128().unsigned_abs(),
            ),
            base_asset_amount: market.base_asset_amount,
            open_interest: market.open_interest,
            total_fee: market.amm.total_fee,
            total_fee_minus_distributions: market.amm.total_fee_minus_distributions,
            adjustment_cost: Number128::new(adjustment_cost),
            oracle_price,
            trade_record: Uint128::zero(),
        },
    )?;
    Ok(Response::new().add_attribute("method", "try_repeg_amm_curve"))
}

pub fn try_update_amm_oracle_twap(
    deps: DepsMut,
    env: Env,
    market_index: u64,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let mut market = MARKETS.load(deps.storage, market_index)?;
    // todo get_oracle_twap is not defined yet
    let oracle_twap = market.amm.get_oracle_twap()?;

    if let Some(oracle_twap) = oracle_twap {
        let oracle_mark_gap_before = (market.amm.last_mark_price_twap.u128() as i128)
            .checked_sub(market.amm.last_oracle_price_twap.i128())
            .ok_or_else(|| (ContractError::MathError))?;

        let oracle_mark_gap_after = (market.amm.last_mark_price_twap.u128() as i128)
            .checked_sub(oracle_twap)
            .ok_or_else(|| (ContractError::MathError))?;

        if (oracle_mark_gap_after > 0 && oracle_mark_gap_before < 0)
            || (oracle_mark_gap_after < 0 && oracle_mark_gap_before > 0)
        {
            market.amm.last_oracle_price_twap =
                Number128::new(market.amm.last_mark_price_twap.u128() as i128);
            market.amm.last_oracle_price_twap_ts = now;
        } else if oracle_mark_gap_after.unsigned_abs() <= oracle_mark_gap_before.unsigned_abs() {
            market.amm.last_oracle_price_twap = Number128::new(oracle_twap);
            market.amm.last_oracle_price_twap_ts = now;
        } else {
            return Err(ContractError::OracleMarkSpreadLimit.into());
        }
    } else {
        return Err(ContractError::InvalidOracle.into());
    }

    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> { Ok(market) },
    )?;

    Ok(Response::new().add_attribute("method", "try_update_amm_oracle_twap"))
}

pub fn try_reset_amm_oracle_twap(
    deps: DepsMut,
    env: Env,
    market_index: u64,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let mut market = MARKETS.load(deps.storage, market_index)?;
    let oracle_guard_rails = ORACLEGUARDRAILS.load(deps.storage)?;
    let oracle_price_data = market.amm.get_oracle_price()?;

    let is_oracle_valid =
        helpers::amm::is_oracle_valid(&market.amm, &oracle_price_data, &oracle_guard_rails)?;

    if !is_oracle_valid {
        market.amm.last_oracle_price_twap =
            Number128::new(market.amm.last_mark_price_twap.u128() as i128);
        market.amm.last_oracle_price_twap_ts = now;
    }
    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> { Ok(market) },
    )?;
    Ok(Response::new().add_attribute("method", "try_reset_amm_oracle_twap"))
}

pub fn try_settle_funding_payment(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let user_address = info.sender;

    controller::funding::settle_funding_payment(&mut deps, &user_address, now)?;
    Ok(Response::new().add_attribute("method", "try_settle_funding_payment"))
}
pub fn try_update_funding_rate(
    mut deps: DepsMut,
    env: Env,
    market_index: u64,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let funding_paused = STATE.load(deps.storage).unwrap().funding_paused;
    controller::funding::update_funding_rate(
        &mut deps,
        market_index,
        now,
        funding_paused,
        None,
    )?;
    Ok(Response::new().add_attribute("method", "try_update_funding_rate"))
}

pub fn try_update_k(
    mut deps: DepsMut,
    env: Env,
    market_index: u64,
    sqrt_k: Uint128,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let mut market = MARKETS.load(deps.storage, market_index)?;

    let base_asset_amount_long = Uint128::from(market.base_asset_amount_long.i128().unsigned_abs());
    let base_asset_amount_short =
        Uint128::from(market.base_asset_amount_short.i128().unsigned_abs());
    let base_asset_amount = market.base_asset_amount.i128().clone();
    let open_interest = market.open_interest.clone();

    let price_before = helpers::amm::calculate_price(
        market.amm.quote_asset_reserve,
        market.amm.base_asset_reserve,
        market.amm.peg_multiplier,
    )?;

    let peg_multiplier_before = market.amm.peg_multiplier;
    let base_asset_reserve_before = market.amm.base_asset_reserve;
    let quote_asset_reserve_before = market.amm.quote_asset_reserve;
    let sqrt_k_before = market.amm.sqrt_k;

    let adjustment_cost = controller::amm::adjust_k_cost(&mut deps, market_index, sqrt_k)?;

    if adjustment_cost > 0 {
        let max_cost = market
            .amm
            .total_fee_minus_distributions
            .checked_sub(market.amm.total_fee_withdrawn)?;
        if adjustment_cost.unsigned_abs() > max_cost.u128() {
            return Err(ContractError::InvalidUpdateK.into());
        } else {
            market.amm.total_fee_minus_distributions = market
                .amm
                .total_fee_minus_distributions
                .checked_sub(Uint128::from(adjustment_cost.unsigned_abs()))?;
        }
    } else {
        market.amm.total_fee_minus_distributions = market
            .amm
            .total_fee_minus_distributions
            .checked_add(Uint128::from(adjustment_cost.unsigned_abs()))?;
    }

    let amm = &market.amm;
    let price_after = helpers::amm::calculate_price(
        amm.quote_asset_reserve,
        amm.base_asset_reserve,
        amm.peg_multiplier,
    )?;

    let price_change_too_large = (price_before.u128() as i128)
        .checked_sub(price_after.u128() as i128)
        .ok_or_else(|| ContractError::MathError {})?
        .unsigned_abs()
        .gt(&UPDATE_K_ALLOWED_PRICE_CHANGE.u128());

    if price_change_too_large {
        return Err(ContractError::InvalidUpdateK.into());
    }

    let peg_multiplier_after = amm.peg_multiplier;
    let base_asset_reserve_after = amm.base_asset_reserve;
    let quote_asset_reserve_after = amm.quote_asset_reserve;
    let sqrt_k_after = amm.sqrt_k;

    let total_fee = amm.total_fee;
    let total_fee_minus_distributions = amm.total_fee_minus_distributions;
    let curve_history_info_length = CURVE_HISTORY_INFO
        .load(deps.storage)?
        .len
        .checked_add(1)
        .ok_or_else(|| (ContractError::MathError))?;
    CURVE_HISTORY_INFO.update(
        deps.storage,
        |mut i: CurveInfo| -> Result<CurveInfo, ContractError> {
            i.len = curve_history_info_length;
            Ok(i)
        },
    )?;

    let OraclePriceData {
        price: oracle_price,
        ..
    } = market.amm.get_oracle_price()?;

    CURVEHISTORY.save(
        deps.storage,
        curve_history_info_length as u64,
        &CurveRecord {
            ts: now,
            record_id: curve_history_info_length,
            market_index,
            peg_multiplier_before,
            base_asset_reserve_before,
            quote_asset_reserve_before,
            sqrt_k_before,
            peg_multiplier_after,
            base_asset_reserve_after,
            quote_asset_reserve_after,
            sqrt_k_after,
            base_asset_amount_long,
            base_asset_amount_short,
            base_asset_amount: Number128::new(base_asset_amount),
            open_interest,
            adjustment_cost: Number128::new(adjustment_cost),
            total_fee,
            total_fee_minus_distributions,
            oracle_price,
            trade_record: Uint128::zero(),
        },
    )?;
    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> { Ok(market) },
    )?;
    Ok(Response::new().add_attribute("method", "try_update_k"))
}

pub fn try_update_margin_ratio(
    deps: DepsMut,
    info: MessageInfo,
    market_index: u64,
    margin_ratio_initial: u32,
    margin_ratio_partial: u32,
    margin_ratio_maintenance: u32,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    helpers::margin_validation::validate_margin(
        margin_ratio_initial,
        margin_ratio_partial,
        margin_ratio_maintenance,
    )?;
    let mut market = MARKETS.load(deps.storage, market_index)?;
    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> {
            market.margin_ratio_initial = margin_ratio_initial;
            market.margin_ratio_partial = margin_ratio_partial;
            market.margin_ratio_maintenance = margin_ratio_maintenance;
            Ok(market)
        },
    )?;
    Ok(Response::new().add_attribute("method", "try_update_margin_ratio"))
}

pub fn try_update_partial_liquidation_close_percentage(
    deps: DepsMut,
    info: MessageInfo,
    value: Decimal,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.partial_liquidation_close_percentage = value;
        Ok(state)
    })?;

    Ok(Response::new().add_attribute("method", "try_update_partial_liquidation_close_percentage"))
}

pub fn try_update_partial_liquidation_penalty_percentage(
    deps: DepsMut,
    info: MessageInfo,
    value: Decimal,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.partial_liquidation_penalty_percentage = value;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute(
        "method",
        "try_update_partial_liquidation_penalty_percentage",
    ))
}

pub fn try_update_full_liquidation_penalty_percentage(
    deps: DepsMut,
    info: MessageInfo,
    value: Decimal,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.full_liquidation_penalty_percentage = value;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "try_update_full_liquidation_penalty_percentage"))
}

pub fn try_update_partial_liquidation_liquidator_share_denominator(
    deps: DepsMut,
    info: MessageInfo,
    denominator: u64,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.partial_liquidation_liquidator_share_denominator = denominator;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute(
        "method",
        "try_update_partial_liquidation_liquidator_share_denominator",
    ))
}

pub fn try_update_full_liquidation_liquidator_share_denominator(
    deps: DepsMut,
    info: MessageInfo,
    denominator: u64,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.full_liquidation_liquidator_share_denominator = denominator;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute(
        "method",
        "try_update_full_liquidation_liquidator_share_denominator",
    ))
}

pub fn try_update_fee(
    deps: DepsMut,
    info: MessageInfo,
    fee: Decimal,
    first_tier_minimum_balance: Uint128,
    first_tier_discount: Decimal,
    second_tier_minimum_balance: Uint128,
    second_tier_discount: Decimal,
    third_tier_minimum_balance: Uint128,
    third_tier_discount: Decimal,
    fourth_tier_minimum_balance: Uint128,
    fourth_tier_discount: Decimal,
    referrer_reward: Decimal,
    referee_discount: Decimal,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let fee_structure = FeeStructure {
        fee,
        first_tier_minimum_balance,
        second_tier_minimum_balance,
        third_tier_minimum_balance,
        fourth_tier_minimum_balance,
        first_tier_discount,
        second_tier_discount,
        third_tier_discount,
        fourth_tier_discount,
        referrer_reward,
        referee_discount,
    };
    FEESTRUCTURE.update(
        deps.storage,
        |mut _f| -> Result<FeeStructure, ContractError> { Ok(fee_structure) },
    )?;
    Ok(Response::new().add_attribute("method", "try_update_fee"))
}

pub fn try_update_order_state_structure(
    deps: DepsMut,
    info: MessageInfo,
    min_order_quote_asset_amount: Uint128,
    reward: Decimal,
    time_based_reward_lower_bound: Uint128,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let order_state = OrderState {
        min_order_quote_asset_amount,
        reward,
        time_based_reward_lower_bound,
    };
    ORDERSTATE.update(deps.storage, |mut _s| -> Result<OrderState, ContractError> {
        Ok(order_state)
    })?;
    Ok(Response::new().add_attribute("method", "try_update_order_filler_reward_structure"))
}

pub fn try_update_market_oracle(
    deps: DepsMut,
    info: MessageInfo,
    market_index: u64,
    oracle: String,
    oracle_source: OracleSource,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let mut market = MARKETS.load(deps.storage, market_index)?;
    market.amm.oracle = addr_validate_to_lower(deps.api, &oracle)?;
    market.amm.oracle_source = oracle_source;
    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> { Ok(market) },
    )?;
    Ok(Response::new().add_attribute("method", "try_update_market_oracle"))
}

pub fn try_update_oracle_guard_rails(
    deps: DepsMut,
    info: MessageInfo,
    use_for_liquidations: bool,
    mark_oracle_divergence: Decimal,
    slots_before_stale: i64,
    confidence_interval_max_size: Uint128,
    too_volatile_ratio: i128,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let oracle_gr = OracleGuardRails {
        use_for_liquidations,
        mark_oracle_divergence,
        slots_before_stale,
        confidence_interval_max_size,
        too_volatile_ratio: Number128::new(too_volatile_ratio),
    };
    ORACLEGUARDRAILS.update(
        deps.storage,
        |mut _o| -> Result<OracleGuardRails, ContractError> { Ok(oracle_gr) },
    )?;

    Ok(Response::new().add_attribute("method", "try_update_oracle_guard_rails"))
}

pub fn try_update_max_deposit(
    deps: DepsMut,
    info: MessageInfo,
    max_deposit: Uint128,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.max_deposit = max_deposit;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "try_max_deposit"))
}

pub fn try_update_exchange_paused(
    deps: DepsMut,
    info: MessageInfo,
    exchange_paused: bool,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.exchange_paused = exchange_paused;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "try_exchange_paused"))
}

pub fn try_disable_admin_control_prices(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.admin_controls_prices = false;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "try_disable_admin_control_prices"))
}
pub fn try_update_funding_paused(
    deps: DepsMut,
    info: MessageInfo,
    funding_paused: bool,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.funding_paused = funding_paused;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "try_update_funding_paused"))
}

pub fn try_update_market_minimum_quote_asset_trade_size(
    deps: DepsMut,
    info: MessageInfo,
    market_index: u64,
    minimum_trade_size: Uint128,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let mut market = MARKETS.load(deps.storage, market_index)?;
    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> {
            market.amm.minimum_quote_asset_trade_size = minimum_trade_size;
            Ok(market)
        },
    )?;
    Ok(Response::new().add_attribute("method", "try_update_market_minimum_quote_asset_trade_size"))
}

pub fn try_update_market_minimum_base_asset_trade_size(
    deps: DepsMut,
    info: MessageInfo,
    market_index: u64,
    minimum_trade_size: Uint128,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let mut market = MARKETS.load(deps.storage, market_index)?;
    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> {
            market.amm.minimum_base_asset_trade_size = minimum_trade_size;
            Ok(market)
        },
    )?;
    Ok(Response::new().add_attribute("method", "try_update_market_minimum_base_asset_trade_size"))
}

pub fn try_update_oracle_address(
    deps: DepsMut,
    info: MessageInfo,
    oracle: String,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let mut state = STATE.load(deps.storage)?;
    state.oracle = addr_validate_to_lower(deps.api, &oracle)?;
    STATE.update(deps.storage, |_state| -> Result<State, ContractError> {
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "try_update_oracle_address"))
}

pub fn try_feeding_price(
    deps: DepsMut,
    info: MessageInfo,
    market_index: u64,
    price: i128,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender.clone())?;
    let mut market = MARKETS.load(deps.storage, market_index)?;
    market.amm.last_oracle_price = Number128::new(price);
    market.amm.last_oracle_price_twap = Number128::new(price);
    MARKETS.update(
        deps.storage,
        market_index,
        |_m| -> Result<Market, ContractError> { Ok(market) },
    )?;
    Ok(Response::new().add_attribute("method", "try_update_oracle_address"))
}
