mod certificate_attributes;
mod certificate_template;

pub use self::{
    certificate_attributes::CertificateAttributes, certificate_template::CertificateTemplate,
};

#[cfg(test)]
pub mod tests {
    pub use super::certificate_attributes::tests::MockCertificateAttributes;
}
