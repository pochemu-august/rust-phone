use crate::error::{Result, SdpError};
use crate::media::{Attribute, Media};
use crate::session::{Origin, SessionDescription};

/// Performs RFC 3264 Offer/Answer negotiation
pub fn negotiate_answer(
    offer: &SessionDescription,
    local_caps: &SessionDescription,
) -> Result<SessionDescription> {
    let mut answer_medias = Vec::new();

    for offer_media in &offer.medias {
        // Find matching local media of the same media_type (e.g. "audio")
        let matching_local = local_caps
            .medias
            .iter()
            .find(|lm| lm.media_type.eq_ignore_ascii_case(&offer_media.media_type));

        if let Some(local_media) = matching_local {
            // Find intersecting formats
            let mut common_formats = Vec::new();
            for fmt in &offer_media.formats {
                if local_media.formats.contains(fmt) {
                    common_formats.push(fmt.clone());
                }
            }

            if common_formats.is_empty() {
                // Reject media by setting port to 0
                let mut rejected = Media::new_audio(0, offer_media.formats.clone());
                rejected.media_type = offer_media.media_type.clone();
                rejected.proto = offer_media.proto.clone();
                answer_medias.push(rejected);
            } else {
                let mut answer_media = Media {
                    media_type: offer_media.media_type.clone(),
                    port: local_media.port,
                    port_count: None,
                    proto: offer_media.proto.clone(),
                    formats: common_formats.clone(),
                    connection: local_media.connection.clone(),
                    attributes: Vec::new(),
                };

                // Copy rtpmap and fmtp for the negotiated formats from local_media or offer_media
                for fmt in &common_formats {
                    for attr in &offer_media.attributes {
                        if (attr.name.eq_ignore_ascii_case("rtpmap")
                            || attr.name.eq_ignore_ascii_case("fmtp"))
                            && attr
                                .value
                                .as_deref()
                                .is_some_and(|val| val.starts_with(fmt))
                        {
                            answer_media.attributes.push(attr.clone());
                        }
                    }
                }

                // Negotiate direction
                let offer_direction = offer_media
                    .attributes
                    .iter()
                    .find(|a| {
                        matches!(
                            a.name.as_str(),
                            "sendrecv" | "sendonly" | "recvonly" | "inactive"
                        )
                    })
                    .map(|a| a.name.as_str())
                    .unwrap_or("sendrecv");

                let answer_direction = match offer_direction {
                    "sendrecv" => "sendrecv",
                    "sendonly" => "recvonly",
                    "recvonly" => "sendonly",
                    "inactive" => "inactive",
                    _ => "sendrecv",
                };
                answer_media
                    .attributes
                    .push(Attribute::flag(answer_direction));

                answer_medias.push(answer_media);
            }
        } else {
            // Media type not supported locally, reject with port 0
            let mut rejected = Media::new_audio(0, offer_media.formats.clone());
            rejected.media_type = offer_media.media_type.clone();
            rejected.proto = offer_media.proto.clone();
            answer_medias.push(rejected);
        }
    }

    if answer_medias.is_empty() {
        return Err(SdpError::NegotiationFailed("No medias in offer".into()));
    }

    let origin = Origin::new(
        local_caps.origin.username.clone(),
        local_caps.origin.sess_id,
        local_caps.origin.sess_version + 1,
        local_caps.origin.unicast_address.clone(),
    );

    let mut answer = SessionDescription::new(origin, "-");
    answer.connection = local_caps.connection.clone();
    answer.time = offer.time.clone();
    answer.medias = answer_medias;

    Ok(answer)
}
