#[cfg(test)]
mod reqwest_wakanda_http_service {

    use bytes::Bytes;

    use load_balancer::wakanda_http_service::reqwest_http_service::ReqwestHttpService;
    use load_balancer::wakanda_http_service::wakanda_http_service::WakandaHttpService;
    use load_balancer::wakanda_http_service::wakanda_http_service_request::{
        WakandaHttpServiceHeaders, WakandaHttpServiceRequest, WakandaHttpServiceRequestHttpMethod,
    };

    use load_balancer::wakanda_http_service::wakanda_http_service_response::WakandaHttpServiceError;
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

        let wakanda_http_service = ReqwestHttpService::default();

        let wakanda_http_service_request = WakandaHttpServiceRequest {
            url: format!("{}{}", mock_server.uri(), "/v1/api/user".to_string()),
            method: WakandaHttpServiceRequestHttpMethod::Get,
            headers: WakandaHttpServiceHeaders::from([
                ("Authorization".to_string(), "Bearer secret".to_string()),
                ("Content-Type".to_string(), "text/plain".to_string()),
            ]),
            body: Bytes::new(),
        };

        let wakanda_http_service_response = wakanda_http_service
            .execute(wakanda_http_service_request)
            .await
            .unwrap();

        assert_eq!(wakanda_http_service_response.status, 200);
        assert_eq!(wakanda_http_service_response.body, Bytes::from("OK"));
        assert_eq!(
            wakanda_http_service_response
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

        let wakanda_http_service = ReqwestHttpService::default();
        let wakanda_http_service_request = WakandaHttpServiceRequest {
            url: format!("{}{}", mock_server.uri(), "/api/data".to_string()),
            method: WakandaHttpServiceRequestHttpMethod::Post,
            headers: WakandaHttpServiceHeaders::from([
                ("Authorization".to_string(), "Bearer secret".to_string()),
                ("Content-Type".to_string(), "text/plain".to_string()),
            ]),
            body: Bytes::from("OK"),
        };

        let wakanda_http_service_response = wakanda_http_service
            .execute(wakanda_http_service_request)
            .await
            .unwrap();

        assert_eq!(wakanda_http_service_response.status, 201);
        assert_eq!(
            wakanda_http_service_response
                .headers
                .get("x-request-id")
                .unwrap(),
            "12345"
        );
        assert_eq!(wakanda_http_service_response.body, Bytes::from("Created"));
    }

    #[tokio::test]
    async fn should_detect_a_network_error() {
        let wakanda_http_service = ReqwestHttpService::default();
        let wakanda_http_service_request = WakandaHttpServiceRequest {
            url: format!(
                "{}{}",
                "http://unknown:1234".to_string(),
                "/health".to_string()
            ),
            method: WakandaHttpServiceRequestHttpMethod::Get,
            headers: WakandaHttpServiceHeaders::default(),
            body: Bytes::new(),
        };

        let wakanda_http_service_response = wakanda_http_service
            .execute(wakanda_http_service_request)
            .await;

        assert!(wakanda_http_service_response.is_err());
        assert!(matches!(
            wakanda_http_service_response.unwrap_err(),
            WakandaHttpServiceError::Network(_)
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

        let wakanda_http_service = ReqwestHttpService::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(1))
                .build()
                .unwrap(),
        );

        let wakanda_http_service_request = WakandaHttpServiceRequest {
            url: format!("{}{}", mock_server.uri(), "/slow".to_string()),
            method: WakandaHttpServiceRequestHttpMethod::Get,
            headers: WakandaHttpServiceHeaders::default(),
            body: Bytes::new(),
        };

        let wakanda_http_service_response = wakanda_http_service
            .execute(wakanda_http_service_request)
            .await;

        assert!(wakanda_http_service_response.is_err());
        assert!(matches!(
            wakanda_http_service_response.unwrap_err(),
            WakandaHttpServiceError::Timeout
        ));
    }

    #[tokio::test]
    async fn should_support_many_http_methods() {
        let mock_server = MockServer::start().await;

        for (method_enum, method_str) in [
            (WakandaHttpServiceRequestHttpMethod::Get, "GET"),
            (WakandaHttpServiceRequestHttpMethod::Post, "POST"),
            (WakandaHttpServiceRequestHttpMethod::Put, "PUT"),
            (WakandaHttpServiceRequestHttpMethod::Delete, "DELETE"),
            (WakandaHttpServiceRequestHttpMethod::Patch, "PATCH"),
        ] {
            Mock::given(method(method_str))
                .and(path("/health"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let wakanda_http_service = ReqwestHttpService::default();
            let wakanda_http_service_request = WakandaHttpServiceRequest {
                url: format!("{}{}", mock_server.uri(), "/health".to_string()),
                method: method_enum,
                headers: WakandaHttpServiceHeaders::default(),
                body: Bytes::new(),
            };

            let wakanda_http_service_response = wakanda_http_service
                .execute(wakanda_http_service_request)
                .await
                .unwrap();

            assert_eq!(wakanda_http_service_response.status, 200);
        }
    }
}
