use near_jsonrpc_primitives::types::query::{QueryResponseKind, RpcQueryError};

use crate::{api_models, errors, types, utils};

pub(crate) async fn get_ft_balance(
    rpc_client: &near_jsonrpc_client::JsonRpcClient,
    contract_id: near_primitives::types::AccountId,
    account_id: near_primitives::types::AccountId,
    block_height: u64,
) -> api_models::Result<u128> {
    let request = get_function_call_request(
        block_height,
        contract_id.clone(),
        "ft_balance_of",
        serde_json::json!({ "account_id": account_id }),
    );
    let response = wrapped_call(rpc_client, request, block_height, &contract_id).await?;
    Ok(serde_json::from_slice::<types::U128>(&response.result)?.0)
}

pub(crate) async fn get_ft_metadata(
    rpc_client: &near_jsonrpc_client::JsonRpcClient,
    contract_id: near_primitives::types::AccountId,
    block_height: u64,
) -> api_models::Result<api_models::FtContractMetadata> {
    let request = get_function_call_request(
        block_height,
        contract_id.clone(),
        "ft_metadata",
        serde_json::json!({}),
    );
    let response = wrapped_call(rpc_client, request, block_height, &contract_id).await?;

    let metadata = serde_json::from_slice::<types::FungibleTokenMetadata>(&response.result)?;
    Ok(api_models::FtContractMetadata {
        spec: metadata.spec,
        name: metadata.name,
        symbol: metadata.symbol,
        icon: metadata.icon,
        decimals: metadata.decimals,
        reference: metadata.reference,
        reference_hash: utils::base64_to_string(&metadata.reference_hash)?,
    })
}

pub(crate) async fn get_nft_general_metadata(
    rpc_client: &near_jsonrpc_client::JsonRpcClient,
    contract_id: near_primitives::types::AccountId,
    block_height: u64,
) -> api_models::Result<api_models::NftContractMetadata> {
    let request = get_function_call_request(
        block_height,
        contract_id.clone(),
        "nft_metadata",
        serde_json::json!({}),
    );
    let response = wrapped_call(rpc_client, request, block_height, &contract_id).await?;

    api_models::NftContractMetadata::try_from(serde_json::from_slice::<types::NFTContractMetadata>(
        &response.result,
    )?)
}

pub(crate) async fn get_nft_count(
    rpc_client: &near_jsonrpc_client::JsonRpcClient,
    contract_id: near_primitives::types::AccountId,
    account_id: near_primitives::types::AccountId,
    block_height: u64,
) -> api_models::Result<u32> {
    let request = get_function_call_request(
        block_height,
        contract_id.clone(),
        "nft_supply_for_owner",
        serde_json::json!({ "account_id": account_id }),
    );
    let response = wrapped_call(rpc_client, request, block_height, &contract_id).await?;

    Ok(serde_json::from_slice::<String>(&response.result)?
        .parse::<u32>()
        .map_err(|e| errors::ErrorKind::InternalError(format!("Failed to parse u32 {}", e)))?)
}

pub(crate) async fn get_nfts(
    rpc_client: &near_jsonrpc_client::JsonRpcClient,
    contract_id: near_primitives::types::AccountId,
    account_id: near_primitives::types::AccountId,
    block_height: u64,
    limit: u32,
) -> api_models::Result<Vec<api_models::NonFungibleToken>> {
    // todo pagination (can wait for phase 2)
    // RPC supports pagination, but the order is defined by the each contract and we can't control it.
    // For now, we are ready to serve only the first page
    // Later, I feel we need to load NFT (each token) metadata to the DB,
    // right after that we can stop using RPC here.
    // Or, maybe we want to delegate this task fully to the contracts?
    let request = get_function_call_request(
        block_height,
        contract_id.clone(),
        "nft_tokens_for_owner",
        // https://nomicon.io/Standards/Tokens/NonFungibleToken/Enumeration
        serde_json::json!({ "account_id": account_id, "from_index": "0", "limit": limit }),
    );
    let response = wrapped_call(rpc_client, request, block_height, &contract_id).await?;

    let tokens = serde_json::from_slice::<Vec<types::Token>>(&response.result)?;
    let mut result = vec![];
    for token in tokens {
        result.push(api_models::NonFungibleToken::try_from(token)?);
    }
    Ok(result)
}

pub(crate) async fn get_nft_metadata(
    rpc_client: &near_jsonrpc_client::JsonRpcClient,
    contract_id: near_primitives::types::AccountId,
    token_id: String,
    block_height: u64,
) -> api_models::Result<api_models::NonFungibleToken> {
    let request = get_function_call_request(
        block_height,
        contract_id.clone(),
        "nft_token",
        serde_json::json!({ "token_id": token_id }),
    );
    let response = wrapped_call(rpc_client, request, block_height, &contract_id).await?;

    match serde_json::from_slice::<Option<types::Token>>(&response.result)? {
        None => Err(errors::ErrorKind::InvalidInput(format!(
            "Token `{}` does not exist in contract `{}`, block_height {}",
            token_id, contract_id, block_height
        ))
        .into()),
        Some(token) => api_models::NonFungibleToken::try_from(token),
    }
}

fn get_function_call_request(
    block_height: u64,
    account_id: near_primitives::types::AccountId,
    method_name: &str,
    args: serde_json::Value,
) -> near_jsonrpc_client::methods::query::RpcQueryRequest {
    near_jsonrpc_client::methods::query::RpcQueryRequest {
        block_reference: near_primitives::types::BlockReference::BlockId(
            near_primitives::types::BlockId::Height(block_height),
        ),
        request: near_primitives::views::QueryRequest::CallFunction {
            account_id,
            method_name: method_name.to_string(),
            args: near_primitives::types::FunctionArgs::from(args.to_string().into_bytes()),
        },
    }
}

async fn wrapped_call(
    rpc_client: &near_jsonrpc_client::JsonRpcClient,
    request: near_jsonrpc_client::methods::query::RpcQueryRequest,
    block_height: u64,
    contract_id: &near_primitives::types::AccountId,
) -> api_models::Result<near_primitives::views::CallResult> {
    match rpc_client.call(request).await {
        Ok(response) => match response.kind {
            QueryResponseKind::CallResult(result) => Ok(result),
            _ => Err(errors::ErrorKind::RPCError(
                "Unexpected type of the response after CallFunction request".to_string(),
            )
            .into()),
        },
        Err(x) => {
            if let Some(RpcQueryError::ContractExecutionError { vm_error, .. }) = x.handler_error()
            {
                if vm_error.contains("CodeDoesNotExist") || vm_error.contains("MethodNotFound") {
                    return Err(errors::contract_not_found(contract_id, block_height).into());
                }
            }
            Err(x.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn init() -> (near_jsonrpc_client::JsonRpcClient, u64) {
        (
            near_jsonrpc_client::JsonRpcClient::connect("https://archival-rpc.mainnet.near.org"),
            68000000,
        )
    }

    #[actix_rt::test]
    async fn test_ft_balance() {
        let (rpc_client, block_height) = init();
        let contract = near_primitives::types::AccountId::from_str("usn").unwrap();
        let account = near_primitives::types::AccountId::from_str("cgarls.near").unwrap();

        let balance = get_ft_balance(&rpc_client, contract, account, block_height)
            .await
            .unwrap();
        assert_eq!(17201878399999996928, balance);
    }

    #[actix_rt::test]
    async fn test_ft_metadata() {
        let (rpc_client, block_height) = init();
        let contract = near_primitives::types::AccountId::from_str("usn").unwrap();

        let metadata = get_ft_metadata(&rpc_client, contract, block_height).await;
        insta::assert_debug_snapshot!(metadata);
    }

    #[actix_rt::test]
    async fn test_ft_metadata_no_contract_deployed() {
        let (rpc_client, block_height) = init();
        let contract = near_primitives::types::AccountId::from_str("olga.near").unwrap();

        let metadata = get_ft_metadata(&rpc_client, contract, block_height).await;
        insta::assert_debug_snapshot!(metadata);
    }

    #[actix_rt::test]
    async fn test_ft_metadata_other_contract_deployed() {
        let (rpc_client, block_height) = init();
        let contract = near_primitives::types::AccountId::from_str("comic.paras.near").unwrap();

        let metadata = get_ft_metadata(&rpc_client, contract, block_height).await;
        insta::assert_debug_snapshot!(metadata);
    }

    #[actix_rt::test]
    async fn test_nft_general_metadata() {
        let (rpc_client, block_height) = init();
        let contract = near_primitives::types::AccountId::from_str("comic.paras.near").unwrap();

        let metadata = get_nft_general_metadata(&rpc_client, contract, block_height).await;
        insta::assert_debug_snapshot!(metadata);
    }

    #[actix_rt::test]
    async fn test_nft_general_metadata_no_contract_deployed() {
        let (rpc_client, block_height) = init();
        let contract = near_primitives::types::AccountId::from_str("olga.near").unwrap();

        let metadata = get_nft_general_metadata(&rpc_client, contract, block_height).await;
        insta::assert_debug_snapshot!(metadata);
    }

    #[actix_rt::test]
    async fn test_nft_general_metadata_other_contract_deployed() {
        let (rpc_client, block_height) = init();
        let contract = near_primitives::types::AccountId::from_str("usn").unwrap();

        let metadata = get_nft_general_metadata(&rpc_client, contract, block_height).await;
        insta::assert_debug_snapshot!(metadata);
    }

    #[actix_rt::test]
    async fn test_nft_list() {
        let (rpc_client, block_height) = init();
        let contract =
            near_primitives::types::AccountId::from_str("billionairebullsclub.near").unwrap();
        let account = near_primitives::types::AccountId::from_str("olenavorobei.near").unwrap();

        let nfts = get_nfts(&rpc_client, contract, account, block_height, 4).await;
        insta::assert_debug_snapshot!(nfts);
    }

    #[actix_rt::test]
    async fn test_nft_metadata() {
        let (rpc_client, block_height) = init();
        let contract = near_primitives::types::AccountId::from_str("x.paras.near").unwrap();
        let token = "415815:1".to_string();

        let nft = get_nft_metadata(&rpc_client, contract, token, block_height).await;
        insta::assert_debug_snapshot!(nft);
    }

    #[actix_rt::test]
    async fn test_nft_metadata_token_does_not_exist() {
        let (rpc_client, block_height) = init();
        let contract = near_primitives::types::AccountId::from_str("x.paras.near").unwrap();
        let token = "no_such_token".to_string();

        let nft = get_nft_metadata(&rpc_client, contract, token, block_height).await;
        insta::assert_debug_snapshot!(nft);
    }
}