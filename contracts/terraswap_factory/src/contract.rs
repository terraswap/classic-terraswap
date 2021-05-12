#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Reply, ReplyOn, Response,
    StdError, StdResult, SubMsg, WasmMsg,
};

use crate::querier::query_liquidity_token;
use crate::response::MsgInstantiateContractResponse;
use crate::state::{pair_key, read_pairs, Config, TmpPairInfo, CONFIG, PAIRS, TMP_PAIR_INFO};

use protobuf::Message;
use terraswap::asset::{AssetInfo, PairInfo, PairInfoRaw};
use terraswap::factory::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, PairsResponse, QueryMsg,
};
use terraswap::pair::InstantiateMsg as PairInstantiateMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        owner: deps.api.addr_canonicalize(info.sender.as_str())?,
        token_code_id: msg.token_code_id,
        pair_code_id: msg.pair_code_id,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response {
        messages: vec![],
        attributes: vec![],
        submessages: vec![],
        data: None,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig {
            owner,
            token_code_id,
            pair_code_id,
        } => execute_update_config(deps, env, info, owner, token_code_id, pair_code_id),
        ExecuteMsg::CreatePair { asset_infos } => execute_create_pair(deps, env, info, asset_infos),
    }
}

// Only owner can execute it
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    token_code_id: Option<u64>,
    pair_code_id: Option<u64>,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        // validate address format
        let _ = deps.api.addr_validate(&owner)?;

        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(token_code_id) = token_code_id {
        config.token_code_id = token_code_id;
    }

    if let Some(pair_code_id) = pair_code_id {
        config.pair_code_id = pair_code_id;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response {
        messages: vec![],
        attributes: vec![attr("action", "update_config")],
        data: None,
        submessages: vec![],
    })
}

// Anyone can execute it to create swap pair
pub fn execute_create_pair(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    asset_infos: [AssetInfo; 2],
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    let raw_infos = [
        asset_infos[0].to_raw(deps.as_ref())?,
        asset_infos[1].to_raw(deps.as_ref())?,
    ];

    let pair_key = pair_key(&raw_infos);
    if let Ok(Some(_)) = PAIRS.may_load(deps.storage, &pair_key) {
        return Err(StdError::generic_err("Pair already exists"));
    }

    TMP_PAIR_INFO.save(
        deps.storage,
        &TmpPairInfo {
            pair_key,
            asset_infos: raw_infos,
        },
    )?;

    Ok(Response {
        messages: vec![],
        attributes: vec![
            attr("action", "create_pair"),
            attr("pair", format!("{}-{}", asset_infos[0], asset_infos[1])),
        ],
        data: None,
        submessages: vec![SubMsg {
            id: 1,
            gas_limit: None,
            msg: WasmMsg::Instantiate {
                code_id: config.pair_code_id,
                send: vec![],
                admin: None,
                label: "".to_string(),
                msg: to_binary(&PairInstantiateMsg {
                    asset_infos,
                    token_code_id: config.token_code_id,
                })?,
            }
            .into(),
            reply_on: ReplyOn::Success,
        }],
    })
}

/// This just stores the result for future query
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    let tmp_pair_info = TMP_PAIR_INFO.load(deps.storage)?;

    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(msg.result.unwrap().data.unwrap().as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;

    let pair_contract = res.get_contract_address();
    let liquidity_token = query_liquidity_token(deps.as_ref(), Addr::unchecked(pair_contract))?;

    PAIRS.save(
        deps.storage,
        &tmp_pair_info.pair_key,
        &PairInfoRaw {
            liquidity_token: deps.api.addr_canonicalize(liquidity_token.as_str())?,
            contract_addr: deps.api.addr_canonicalize(pair_contract)?,
            asset_infos: tmp_pair_info.asset_infos,
        },
    )?;

    Ok(Response {
        messages: vec![],
        submessages: vec![],
        attributes: vec![
            attr("pair_contract_addr", pair_contract),
            attr("liquidity_token_addr", liquidity_token.as_str()),
        ],
        data: None,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Pair { asset_infos } => to_binary(&query_pair(deps, asset_infos)?),
        QueryMsg::Pairs { start_after, limit } => {
            to_binary(&query_pairs(deps, start_after, limit)?)
        }
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state: Config = CONFIG.load(deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.addr_humanize(&state.owner)?.to_string(),
        token_code_id: state.token_code_id,
        pair_code_id: state.pair_code_id,
    };

    Ok(resp)
}

pub fn query_pair(deps: Deps, asset_infos: [AssetInfo; 2]) -> StdResult<PairInfo> {
    let pair_key = pair_key(&[asset_infos[0].to_raw(deps)?, asset_infos[1].to_raw(deps)?]);
    let pair_info: PairInfoRaw = PAIRS.load(deps.storage, &pair_key)?;
    pair_info.to_normal(deps.api)
}

pub fn query_pairs(
    deps: Deps,
    start_after: Option<[AssetInfo; 2]>,
    limit: Option<u32>,
) -> StdResult<PairsResponse> {
    let start_after = if let Some(start_after) = start_after {
        Some([start_after[0].to_raw(deps)?, start_after[1].to_raw(deps)?])
    } else {
        None
    };

    let pairs: Vec<PairInfo> = read_pairs(deps.storage, deps.api, start_after, limit)?;
    let resp = PairsResponse { pairs };

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
