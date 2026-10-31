pub mod pb {
    tonic::include_proto!("solid.v1");
}

pub mod service;
pub use service::GrpcUserProvisioningService;
