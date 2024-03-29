#[cfg(test)]
mod tests {
    use crate::contract::{instantiate, query, deposit, change_clearing_house, execute};
    use crate::msg::{InstantiateMsg, ConfigResponse, QueryMsg, BalanceResponse, ExecuteMsg};

    
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Uint128, Addr};

    // initlization and verify data
    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            clearing_house: Addr::unchecked("testaddr"),
            denom_stable: "uusd".to_string(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetConfig {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("testaddr"), value.clearing_house);
        assert_eq!("creator", value.admin);
        assert_eq!("uusd", value.denom);
    }

    #[test]
    fn proper_deposit() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            clearing_house: Addr::unchecked("testaddr"),
            denom_stable: "uusd".to_string(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetBalance {}).unwrap();
        let value: BalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(0u64), value.balance);

        // let dep_msg = ExecuteMsg::Deposit{};
        let dep_info = mock_info("testaddr", &coins(1000000, "uusd"));

        deposit(deps.as_mut(), dep_info).unwrap();
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetBalance {}).unwrap();
        let value: BalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(1000000u64), value.balance);
    }

    #[test]
    fn proper_admin_clearing_house_update() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            clearing_house: Addr::unchecked("testaddr"),
            denom_stable: "uusd".to_string(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetConfig {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("testaddr"), value.clearing_house);
        let dep_info = mock_info("creator", &coins(1000000, "uusd"));

        change_clearing_house(
            deps.as_mut(),
            dep_info.clone(),
            Addr::unchecked("newclearing"),
        )
        .unwrap();

        execute(
            deps.as_mut(),
            mock_env(),
            dep_info.clone(),
            ExecuteMsg::UpdateAdmin {
                new_admin: "newadmin".to_string(),
            },
        )
        .unwrap();
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetConfig {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("newclearing"), value.clearing_house);
        assert_eq!("newadmin", value.admin);
    }
}
