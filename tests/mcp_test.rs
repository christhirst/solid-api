use serde_json::json;
use solid_api::mcp::handle_mcp_request;
use solid_api::store::Store;

#[tokio::test]
async fn test_mcp_initialize_list_and_call_tools() {
    let store = Store::new_in_memory().await.expect("in-memory store");

    // 1. Test MCP initialize
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-ai-agent", "version": "1.0.0" }
        }
    });
    let init_resp = handle_mcp_request(&store, init_req)
        .await
        .expect("init response");
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "solid-api-mcp");

    // 2. Test MCP tools/list
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let list_resp = handle_mcp_request(&store, list_req)
        .await
        .expect("tools/list response");
    let tools = list_resp["result"]["tools"]
        .as_array()
        .expect("tools array");
    assert!(tools.len() >= 30);

    // 3. Test provisioning a user via MCP tool call
    let prov_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "provision_user",
            "arguments": {
                "sub": "ai_user_1",
                "username": "ai_operator",
                "role": "admin",
                "namespace": "ai-ops"
            }
        }
    });
    let prov_resp = handle_mcp_request(&store, prov_req)
        .await
        .expect("provision_user response");
    assert_eq!(prov_resp["result"]["isError"], false);
    let prov_text = prov_resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(prov_text.contains("ai_operator"));

    // 4. Test creating a dashboard via MCP tool call
    let dash_req = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "create_dashboard",
            "arguments": {
                "id": "mcp_dash_01",
                "name": "AI Telemetry Dashboard",
                "owner": "ai_user_1",
                "slug": "telemetry",
                "is_public": true,
                "tags": ["ai", "metrics"],
                "buttons": [
                    { "id": "w1", "label": "Latency", "type": "gauge" }
                ]
            }
        }
    });
    let dash_resp = handle_mcp_request(&store, dash_req)
        .await
        .expect("create_dashboard response");
    assert_eq!(dash_resp["result"]["isError"], false);
    let dash_text = dash_resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(dash_text.contains("AI Telemetry Dashboard"));
    assert!(dash_text.contains("ai-ops"));

    // 5. Test uploading a proto and CA cert via MCP tool call
    let proto_req = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "upload_proto",
            "arguments": {
                "name": "telemetry.proto",
                "content": "syntax = \"proto3\"; package telemetry;",
                "owner": "ai_user_1"
            }
        }
    });
    let proto_resp = handle_mcp_request(&store, proto_req)
        .await
        .expect("upload_proto response");
    assert_eq!(proto_resp["result"]["isError"], false);
}
