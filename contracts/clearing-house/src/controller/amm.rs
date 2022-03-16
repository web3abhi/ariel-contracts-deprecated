use crate::error::{ContractError};
use crate::helpers::amm::calculate_quote_asset_amount_swapped;
use crate::helpers::casting::{cast, cast_to_i128};
use crate::helpers::constants::PRICE_TO_PEG_PRECISION_RATIO;
use crate::helpers::{amm, bn, quote_asset::*};

use crate::states::market::Amm;
// use cosmwasm_std::{StdResult, StdError};

use crate::states::state::SwapDirection;

pub fn swap_quote_asset(
    amm: &mut Amm,
    quote_asset_amount: u128,
    direction: SwapDirection,
    now: i64,
    precomputed_mark_price: Option<u128>,
) -> Result<i128, ContractError> {
    amm::update_mark_twap(amm, now, precomputed_mark_price)?;
    let quote_asset_reserve_amount =
        asset_to_reserve_amount(quote_asset_amount, amm.peg_multiplier)?;

    if quote_asset_reserve_amount < amm.minimum_trade_size {
        return Err(ContractError::TradeSizeTooSmall);
    }

    let initial_base_asset_reserve = amm.base_asset_reserve;
    let (new_base_asset_reserve, new_quote_asset_reserve) = amm::calculate_swap_output(
        quote_asset_reserve_amount,
        amm.quote_asset_reserve,
        direction,
        amm.sqrt_k,
    )?;

    amm.base_asset_reserve = new_base_asset_reserve;
    amm.quote_asset_reserve = new_quote_asset_reserve;

    let base_asset_amount = cast_to_i128(initial_base_asset_reserve)?
        .checked_sub(cast(new_base_asset_reserve)?)
        .ok_or_else(Err(ContractError::MathError)?;

    return Ok(base_asset_amount);
}

pub fn swap_base_asset(
    amm: &mut Amm,
    base_asset_swap_amount: u128,
    direction: SwapDirection,
    now: i64,
) -> Result<u128, ContractError> {
    amm::update_mark_twap(amm, now, None)?;

    let initial_quote_asset_reserve = amm.quote_asset_reserve;
    let (new_quote_asset_reserve, new_base_asset_reserve) = amm::calculate_swap_output(
        base_asset_swap_amount,
        amm.base_asset_reserve,
        direction,
        amm.sqrt_k,
    )?;

    amm.base_asset_reserve = new_base_asset_reserve;
    amm.quote_asset_reserve = new_quote_asset_reserve;

    calculate_quote_asset_amount_swapped(
        initial_quote_asset_reserve,
        new_quote_asset_reserve,
        direction,
        amm.peg_multiplier,
    )
}

pub fn move_price(
    amm: &mut Amm,
    base_asset_reserve: u128,
    quote_asset_reserve: u128,
) -> Result<(), ContractError> {
    amm.base_asset_reserve = base_asset_reserve;
    amm.quote_asset_reserve = quote_asset_reserve;

    let k = bn::U256::from(base_asset_reserve)
        .checked_mul(bn::U256::from(quote_asset_reserve))
        .ok_or_else(Err(ContractError::MathError))?;

    amm.sqrt_k = k.integer_sqrt().try_to_u128()?;

    Ok(())
}

#[allow(dead_code)]
pub fn move_to_price(amm: &mut Amm, target_price: u128) -> Result<(), ContractError> {
    let sqrt_k = bn::U256::from(amm.sqrt_k);
    let k = sqrt_k.checked_mul(sqrt_k).ok_or_else(|| (Err(ContractError::MathError)))?;

    let new_base_asset_amount_squared = k
        .checked_mul(bn::U256::from(amm.peg_multiplier))
        .ok_or_else(|| (ContractError::MathError))?
        .checked_mul(bn::U256::from(PRICE_TO_PEG_PRECISION_RATIO))
        .ok_or_else(|| (ContractError::MathError))?
        .checked_div(bn::U256::from(target_price))
        .ok_or_else(|| (ContractError::MathError))?;

    let new_base_asset_amount = new_base_asset_amount_squared.integer_sqrt();
    let new_quote_asset_amount = k
        .checked_div(new_base_asset_amount)
        .ok_or_else(|| (ContractError::MathError))?;

    amm.base_asset_reserve = new_base_asset_amount.try_to_u128()?;
    amm.quote_asset_reserve = new_quote_asset_amount.try_to_u128()?;

    Ok(())
}
