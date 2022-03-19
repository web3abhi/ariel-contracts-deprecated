use std::cell::{Ref, RefMut};

use cosmwasm_std::{Addr, DepsMut};

use crate::states::market::{Market};
use crate::states::user::{User, Position};

use crate::error::ContractError;

use crate::helpers::collateral::calculate_updated_collateral;
use crate::helpers::constants::MARGIN_PRECISION;
use crate::helpers::position::calculate_base_asset_value_and_pnl;


pub fn calculate_margin_ratio(
    deps: DepsMut, 
    userAddress: &Addr,
    markets: &Ref<Market>,
) -> Result<(u128, i128, u128, u128), ContractError> {
    user
    user_positions
    markets
    let mut base_asset_value: u128 = 0;
    let mut unrealized_pnl: i128 = 0;

    // loop 1 to calculate unrealized_pnl
    for market_position in user_positions.positions.iter() {
        if market_position.base_asset_amount == 0 {
            continue;
        }

        let amm = &markets.markets[Market::index_from_u64(market_position.market_index)].amm;
        let (position_base_asset_value, position_unrealized_pnl) =
            calculate_base_asset_value_and_pnl(market_position, amm)?;

        base_asset_value = base_asset_value
            .checked_add(position_base_asset_value)
            .ok_or_else(|| (ContractError::MathError))?;
        unrealized_pnl = unrealized_pnl
            .checked_add(position_unrealized_pnl)
            .ok_or_else(|| (ContractError::MathError))?;
    }

    let total_collateral: u128;
    let margin_ratio: u128;
    if base_asset_value == 0 {
        total_collateral = u128::MAX;
        margin_ratio = u128::MAX;
    } else {
        total_collateral = calculate_updated_collateral(user.collateral, unrealized_pnl)?;
        margin_ratio = total_collateral
            .checked_mul(MARGIN_PRECISION)
            .ok_or_else(|| (ContractError::MathError))?
            .checked_div(base_asset_value)
            .ok_or_else(|| (ContractError::MathError))?;
    }

    Ok((
        total_collateral,
        unrealized_pnl,
        base_asset_value,
        margin_ratio,
    ))
}