#[cfg(test)]
mod reqwest_forward_service {

    use bytes::Bytes;
    use load_balancer::forward_service::forward_service::ForwardService;
    use load_balancer::forward_service::forward_service_request::{
        ForwardServiceRequest, ForwardServiceRequestHeaders, ForwardServiceRequestHttpMethod,
    };
    use load_balancer::forward_service::forward_service_response::ForwardServiceError;
    use load_balancer::forward_service::reqwest_forward_service::ReqwestForwardService;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn should_forward_a_get_request_propagating_the_response() {
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

        let forward_service = ReqwestForwardService::new();

        let forward_service_request = ForwardServiceRequest {
            url: format!("{}{}", mock_server.uri(), "/v1/api/user".to_string()),
            method: ForwardServiceRequestHttpMethod::Get,
            headers: ForwardServiceRequestHeaders::from([
                ("Authorization".to_string(), "Bearer secret".to_string()),
                ("Content-Type".to_string(), "text/plain".to_string()),
            ]),
            body: Bytes::new(),
        };

        let forward_service_response = forward_service
            .execute(forward_service_request)
            .await
            .unwrap();

        assert_eq!(forward_service_response.status, 200);
        assert_eq!(forward_service_response.body, Bytes::from("OK"));
        assert_eq!(
            forward_service_response
                .headers
                .get("x-request-id")
                .unwrap(),
            "12345"
        );
    }

    #[tokio::test]
    async fn should_forward_a_post_request_propagating_the_response() {
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

        let forward_service = ReqwestForwardService::new();
        let forward_service_request = ForwardServiceRequest {
            url: format!("{}{}", mock_server.uri(), "/api/data".to_string()),
            method: ForwardServiceRequestHttpMethod::Post,
            headers: ForwardServiceRequestHeaders::from([
                ("Authorization".to_string(), "Bearer secret".to_string()),
                ("Content-Type".to_string(), "text/plain".to_string()),
            ]),
            body: Bytes::from("OK"),
        };

        let forward_service_response = forward_service
            .execute(forward_service_request)
            .await
            .unwrap();

        assert_eq!(forward_service_response.status, 201);
        assert_eq!(
            forward_service_response
                .headers
                .get("x-request-id")
                .unwrap(),
            "12345"
        );
        assert_eq!(forward_service_response.body, Bytes::from("Created"));
    }

    #[tokio::test]
    async fn should_detect_a_network_error() {
        let forward_service = ReqwestForwardService::new();
        let forward_service_request = ForwardServiceRequest {
            url: format!(
                "{}{}",
                "http://unknown:1234".to_string(),
                "/health".to_string()
            ),
            method: ForwardServiceRequestHttpMethod::Get,
            headers: ForwardServiceRequestHeaders::default(),
            body: Bytes::new(),
        };

        let forward_service_response = forward_service.execute(forward_service_request).await;

        assert!(forward_service_response.is_err());
        assert!(matches!(
            forward_service_response.unwrap_err(),
            ForwardServiceError::Network(_)
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

        let forward_service = ReqwestForwardService::with_client(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(1))
                .build()
                .unwrap(),
        );

        let forward_service_request = ForwardServiceRequest {
            url: format!("{}{}", mock_server.uri(), "/slow".to_string()),
            method: ForwardServiceRequestHttpMethod::Get,
            headers: ForwardServiceRequestHeaders::default(),
            body: Bytes::new(),
        };

        let forward_service_response = forward_service.execute(forward_service_request).await;

        assert!(forward_service_response.is_err());
        assert!(matches!(
            forward_service_response.unwrap_err(),
            ForwardServiceError::Timeout
        ));
    }

    #[tokio::test]
    async fn should_support_many_http_methods() {
        let mock_server = MockServer::start().await;

        for (method_enum, method_str) in [
            (ForwardServiceRequestHttpMethod::Get, "GET"),
            (ForwardServiceRequestHttpMethod::Post, "POST"),
            (ForwardServiceRequestHttpMethod::Put, "PUT"),
            (ForwardServiceRequestHttpMethod::Delete, "DELETE"),
            (ForwardServiceRequestHttpMethod::Patch, "PATCH"),
        ] {
            Mock::given(method(method_str))
                .and(path("/health"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let forward_service = ReqwestForwardService::new();
            let forward_service_request = ForwardServiceRequest {
                url: format!("{}{}", mock_server.uri(), "/health".to_string()),
                method: method_enum,
                headers: ForwardServiceRequestHeaders::default(),
                body: Bytes::new(),
            };

            let forward_service_response = forward_service
                .execute(forward_service_request)
                .await
                .unwrap();

            assert_eq!(forward_service_response.status, 200);
        }
    }
}
