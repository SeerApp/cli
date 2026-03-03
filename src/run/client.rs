use anyhow::Context;
use tonic::{
    transport::{Channel, ClientTlsConfig},
    service::interceptor::InterceptedService,
    Request,
};
use seer_protos_community_neoeinstein_tonic::seer::sessions::v1::tonic::sessions_service_client::SessionsServiceClient;
use seer_protos_community_neoeinstein_prost::seer::sessions::v1::*;

/// Adds `Authorization: Bearer <token>` to every outgoing request.
/// clerk-gate validates this and injects `x-user-id` before forwarding to the service.
#[derive(Clone)]
pub struct BearerInterceptor {
    pub token: tonic::metadata::MetadataValue<tonic::metadata::Ascii>,
}

impl tonic::service::Interceptor for BearerInterceptor {
    fn call(&mut self, mut req: Request<()>) -> Result<Request<()>, tonic::Status> {
        req.metadata_mut()
            .insert("authorization", self.token.clone());
        Ok(req)
    }
}

/// Thin wrapper around the generated gRPC client that handles auth and TLS setup.
pub struct SessionsClient {
    inner: SessionsServiceClient<InterceptedService<Channel, BearerInterceptor>>,
}

impl SessionsClient {
    /// Connect to the sessions gRPC service.
    /// Automatically enables TLS (with system CA roots) for `https://` endpoints.
    pub async fn connect(endpoint_url: &str, token: &str) -> anyhow::Result<Self> {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .ok();

        let mut endpoint = Channel::from_shared(endpoint_url.to_string())
            .context("Invalid gRPC endpoint")?;

        // Enable TLS with system CA roots for https endpoints (e.g. prod)
        if endpoint_url.starts_with("https://") {
            endpoint = endpoint
                .tls_config(ClientTlsConfig::new().with_native_roots())
                .context("Failed to configure TLS")?;
        }

        let channel = endpoint
            .connect()
            .await
            .context("Failed to connect to gRPC server")?;

        let token_val = format!("Bearer {}", token)
            .parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>()
            .map_err(|e| anyhow::anyhow!("Invalid bearer token: {}", e))?;

        let inner = SessionsServiceClient::with_interceptor(
            channel,
            BearerInterceptor { token: token_val },
        );

        Ok(Self { inner })
    }

    pub async fn create_session(
        &mut self,
        request: Request<CreateSessionRequest>,
    ) -> Result<tonic::Response<CreateSessionResponse>, tonic::Status> {
        self.inner.create_session(request).await
    }

    pub async fn run_session(
        &mut self,
        request: Request<RunSessionRequest>,
    ) -> Result<tonic::Response<RunSessionResponse>, tonic::Status> {
        self.inner.run_session(request).await
    }
}
