#[cfg(test)]
mod reqwest_http_client {

    use bytes::Bytes;

    use load_balancer::http_client::error::Error;
    use load_balancer::http_client::reqwest_http_client::ReqwestHttpClient;
    use load_balancer::http_client::http_client::HttpClient;
    use load_balancer::http_client::request::{
        RequestHeaders, Request, RequestMethod,
    };

    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn should_proxy_a_get_request_propagating_the_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/api/user"))
            .and(header("Authorization", "Bearer secret"))
            .and(header("Content-Type", "text/plain"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("X-Request-Id", "12345")
                    .set_body_raw("OK", "application/text"),
            )
            .mount(&mock_server)
            .await;

        let http_client = ReqwestHttpClient::default();

        let http_client_request = Request {
            url: format!("{}{}", mock_server.uri(), "/v1/api/user".to_string()),
            method: RequestMethod::Get,
            headers: RequestHeaders::from([
                ("Authorization".to_string(), "Bearer secret".to_string()),
                ("Content-Type".to_string(), "text/plain".to_string()),
            ]),
            body: Bytes::new(),
        };

        let http_client_response = http_client
            .execute(http_client_request)
            .await
            .unwrap();

        assert_eq!(http_client_response.status, 200);
        assert_eq!(http_client_response.body, Bytes::from("OK"));
        assert_eq!(
            http_client_response
                .headers
                .get("x-request-id")
                .unwrap(),
            "12345"
        );
    }

    #[tokio::test]
    async fn should_proxy_a_post_request_propagating_the_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/data"))
            .and(header("Authorization", "Bearer secret"))
            .and(header("Content-Type", "text/plain"))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("X-Request-Id", "12345")
                    .set_body_string("Created"),
            )
            .mount(&mock_server)
            .await;

        let http_client = ReqwestHttpClient::default();
        let http_client_request = Request {
            url: format!("{}{}", mock_server.uri(), "/api/data".to_string()),
            method: RequestMethod::Post,
            headers: RequestHeaders::from([
                ("Authorization".to_string(), "Bearer secret".to_string()),
                ("Content-Type".to_string(), "text/plain".to_string()),
            ]),
            body: Bytes::from("OK"),
        };

        let http_client_response = http_client
            .execute(http_client_request)
            .await
            .unwrap();

        assert_eq!(http_client_response.status, 201);
        assert_eq!(
            http_client_response
                .headers
                .get("x-request-id")
                .unwrap(),
            "12345"
        );
        assert_eq!(http_client_response.body, Bytes::from("Created"));
    }

    #[tokio::test]
    async fn should_detect_a_network_error() {
        let http_client = ReqwestHttpClient::default();
        let http_client_request = Request {
            url: format!(
                "{}{}",
                "http://unknown:1234".to_string(),
                "/health".to_string()
            ),
            method: RequestMethod::Get,
            headers: RequestHeaders::default(),
            body: Bytes::new(),
        };

        let http_client_response = http_client
            .execute(http_client_request)
            .await;

        assert!(http_client_response.is_err());
        assert!(matches!(
            http_client_response.unwrap_err(),
            Error::Network(_)
        ));
    }

    #[tokio::test]
    async fn should_detect_a_timeout_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(100)),
            )
            .mount(&mock_server)
            .await;

        let http_client = ReqwestHttpClient::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(1))
                .build()
                .unwrap(),
        );

        let http_client_request = Request {
            url: format!("{}{}", mock_server.uri(), "/slow".to_string()),
            method: RequestMethod::Get,
            headers: RequestHeaders::default(),
            body: Bytes::new(),
        };

        let http_client_response = http_client
            .execute(http_client_request)
            .await;

        assert!(http_client_response.is_err());
        assert!(matches!(
            http_client_response.unwrap_err(),
            Error::Timeout
        ));
    }

    #[tokio::test]
    async fn should_support_many_http_methods() {
        let mock_server = MockServer::start().await;

        for (method_enum, method_str) in [
            (RequestMethod::Get, "GET"),
            (RequestMethod::Post, "POST"),
            (RequestMethod::Put, "PUT"),
            (RequestMethod::Delete, "DELETE"),
            (RequestMethod::Patch, "PATCH"),
        ] {
            Mock::given(method(method_str))
                .and(path("/health"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let http_client = ReqwestHttpClient::default();
            let http_client_request = Request {
                url: format!("{}{}", mock_server.uri(), "/health".to_string()),
                method: method_enum,
                headers: RequestHeaders::default(),
                body: Bytes::new(),
            };

            let http_client_response = http_client
                .execute(http_client_request)
                .await
                .unwrap();

            assert_eq!(http_client_response.status, 200);
        }
    }
}
