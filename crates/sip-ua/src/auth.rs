use md5::{Digest, Md5};
use sip_core::AuthHeader;

pub fn md5_hex(data: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    format!("{:02x}", result)
}

#[allow(clippy::too_many_arguments)]
pub fn calculate_digest_response(
    username: &str,
    password: &str,
    realm: &str,
    method: &str,
    digest_uri: &str,
    nonce: &str,
    nc: Option<&str>,
    cnonce: Option<&str>,
    qop: Option<&str>,
) -> String {
    // HA1 = MD5(username:realm:password)
    let ha1 = md5_hex(&format!("{}:{}:{}", username, realm, password));

    // HA2 = MD5(method:digest_uri)
    let ha2 = md5_hex(&format!("{}:{}", method, digest_uri));

    if let (Some(qop_val), Some(nc_val), Some(cnonce_val)) = (qop, nc, cnonce) {
        // response = MD5(HA1:nonce:nc:cnonce:qop:HA2)
        md5_hex(&format!(
            "{}:{}:{}:{}:{}:{}",
            ha1, nonce, nc_val, cnonce_val, qop_val, ha2
        ))
    } else {
        // response = MD5(HA1:nonce:HA2)
        md5_hex(&format!("{}:{}:{}", ha1, nonce, ha2))
    }
}

pub fn create_auth_response(
    challenge: &AuthHeader,
    username: &str,
    password: &str,
    method: &str,
    digest_uri: &str,
) -> AuthHeader {
    let realm = challenge.realm.as_deref().unwrap_or("");
    let nonce = challenge.nonce.as_deref().unwrap_or("");
    let qop_option = challenge.qop.as_deref().and_then(|q| {
        if q.contains("auth") {
            Some("auth")
        } else {
            None
        }
    });

    let (nc, cnonce) = if qop_option.is_some() {
        (Some("00000001"), Some("0a4f113b"))
    } else {
        (None, None)
    };

    let response = calculate_digest_response(
        username, password, realm, method, digest_uri, nonce, nc, cnonce, qop_option,
    );

    let mut auth = AuthHeader::new("Digest");
    auth.username = Some(username.to_string());
    auth.realm = Some(realm.to_string());
    auth.nonce = Some(nonce.to_string());
    auth.uri = Some(digest_uri.to_string());
    auth.response = Some(response);
    auth.algorithm = challenge.algorithm.clone();
    auth.opaque = challenge.opaque.clone();
    if let Some(q) = qop_option {
        auth.qop = Some(q.to_string());
        auth.nc = nc.map(|s| s.to_string());
        auth.cnonce = cnonce.map(|s| s.to_string());
    }

    auth
}
