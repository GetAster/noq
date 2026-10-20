//! Receiving stateless resets (RFC 9000 §10.3.1), including on multipath connections
//!
//! A peer that lost its connection state answers our packets with a stateless reset carrying
//! the token it issued for the CID we sent to. The reset carries no CID we issued, so the
//! endpoint has to find the connection by that token, on whichever path the token belongs to.

use alloc::{boxed::Box, sync::Arc, vec};
use core::net::SocketAddr;

use assert_matches::assert_matches;
#[cfg(all(feature = "aws-lc-rs", not(feature = "ring")))]
use aws_lc_rs::hmac;
use bytes::BytesMut;
use rand::Rng;
#[cfg(feature = "ring")]
use ring::hmac;
use testresult::TestResult;
use tracing::info;

use super::util::{ConnPair, Inbound, ManyToManyRouting, Pair, server_config, subscribe};
use crate::{
    ConnectionError, Duration, Endpoint, EndpointConfig, Event, FourTuple,
    HashedConnectionIdGenerator, MIN_STATELESS_RESET_SIZE, PathEvent, PathId, PathStatus,
    RESET_TOKEN_SIZE, ResetToken, Side::*, VarInt,
};

/// Reset key material shared by the original server endpoint and its "restarted" replacement
const RESET_KEY: [u8; 64] = [0x5a; 64];

fn reset_key() -> hmac::Key {
    hmac::Key::new(hmac::HMAC_SHA256, &RESET_KEY)
}

/// Endpoint config whose reset key and CID validation survive a restart
///
/// Both are needed: the restarted endpoint only answers a packet with a stateless reset when
/// its CID generator accepts the packet's CID as one it could have issued.
fn endpoint_config() -> EndpointConfig {
    let mut config = EndpointConfig::new(Arc::new(reset_key()));
    config.cid_generator(Arc::new(|| {
        Box::new(HashedConnectionIdGenerator::from_key(0))
    }));
    config
}

/// Replaces the server endpoint with a fresh one using the same reset key
///
/// This is a server process that restarted with the same reset key: it knows no connections
/// and answers packets for them with stateless resets the client can validate.
fn restart_server(pair: &mut Pair) {
    pair.server.endpoint = Endpoint::new(
        Arc::new(endpoint_config()),
        Some(Arc::new(server_config())),
        true,
    );
}

/// Delivers packets back and forth for a few milliseconds, far less than any idle timeout
fn exchange(pair: &mut Pair) {
    for _ in 0..4 {
        pair.drive_client();
        pair.drive_server();
        pair.time += Duration::from_millis(1);
    }
    pair.drive_client();
}

/// A datagram shaped like a stateless reset: `len - 16` unpredictable bytes that look like a
/// short header packet, then `token`
fn forged_reset(len: usize, token: &[u8]) -> BytesMut {
    assert_eq!(token.len(), RESET_TOKEN_SIZE);
    let mut datagram = vec![0u8; len - RESET_TOKEN_SIZE];
    rand::rng().fill_bytes(&mut datagram);
    datagram[0] = 0b0100_0000 | (datagram[0] & 0b0011_1111);
    datagram.extend_from_slice(token);
    datagram[..].into()
}

/// Hands `datagram` to the client as if it arrived from `from` on the local address `to`
fn deliver_to_client(pair: &mut Pair, from: SocketAddr, to: SocketAddr, datagram: BytesMut) {
    pair.client.inbound.push(
        pair.time,
        Inbound {
            ecn: None,
            packet: datagram,
            remote: from,
            dst_ip: Some(to.ip()),
        },
    );
    pair.drive_client();
}

/// The token the server issued for the CID the client currently sends to on `path_id`
fn client_token(pair: &ConnPair, path_id: PathId) -> ResetToken {
    let cid = pair
        .conn(Client)
        .active_remote_cid(path_id)
        .expect("path has a remote CID");
    ResetToken::new(&reset_key(), cid)
}

/// Polls `side` until a `ConnectionLost` event, returning its reason
fn lost_reason(pair: &mut ConnPair, side: crate::Side) -> Option<ConnectionError> {
    while let Some(event) = pair.poll(side) {
        if let Event::ConnectionLost { reason } = event {
            return Some(reason);
        }
    }
    None
}

/// Builds a multipath pair with a second address on each side, both routable
fn multipath_pair() -> (ConnPair, SocketAddr, SocketAddr, SocketAddr, SocketAddr) {
    let mut pair = ConnPair::builder()
        .enable_multipath()
        .with_server_endpoint_cfg(endpoint_config())
        .connect();
    let client_addr = pair.routes.as_basic().client_addr;
    let server_addr = pair.routes.as_basic().server_addr;
    // Distinct IPs, so routing can tell which local address a packet leaves from
    let second_client_addr: SocketAddr = "[::1:2]:1".parse().expect("valid address");
    let second_server_addr: SocketAddr = "[::2:2]:1".parse().expect("valid address");
    pair.routes = ManyToManyRouting::simple_symmetric(
        [client_addr, second_client_addr],
        [server_addr, second_server_addr],
    )
    .into();
    (
        pair,
        client_addr,
        server_addr,
        second_client_addr,
        second_server_addr,
    )
}

/// Opens a path from `local` to `remote` and drives until both sides have established it
fn open_second_path(pair: &mut ConnPair, local: SocketAddr, remote: SocketAddr) -> PathId {
    let network_path = FourTuple {
        local_ip: Some(local.ip()),
        remote,
    };
    let path_id = pair
        .open_path(Client, network_path, PathStatus::Available)
        .expect("path opens");
    pair.drive();
    assert_ne!(path_id, PathId::ZERO);
    assert_matches!(
        pair.poll(Client),
        Some(Event::Path(PathEvent::Established { id })) if id == path_id
    );
    assert_matches!(
        pair.poll(Server),
        Some(Event::Path(PathEvent::Established { id })) if id == path_id
    );
    path_id
}

/// A reset whose CID matches nothing closes the connection that holds its token, and only
/// that connection.
#[test]
fn reset_for_unknown_cid_closes_the_connection_holding_the_token() {
    let _guard = subscribe();
    let mut pair = Pair::new(Arc::new(endpoint_config()), server_config());
    let (quiet_ch, _) = pair.connect();
    let (pinged_ch, _) = pair.connect();
    pair.drive();

    restart_server(&mut pair);
    info!("pinging one connection to the restarted server");
    pair.client_conn_mut(pinged_ch).ping();
    exchange(&mut pair);

    assert_matches!(
        pair.client_conn_mut(pinged_ch).poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::Reset
        })
    );
    assert!(!pair.client_conn_mut(quiet_ch).is_closed());
    assert_matches!(pair.client_conn_mut(quiet_ch).poll(), None);
}

/// A datagram whose tail is not a token we hold, or that is shorter than the smallest
/// possible stateless reset, does nothing; the same token in a datagram of the minimum size
/// closes the connection.
#[test]
fn reset_with_invalid_token_or_too_short_is_ignored() -> TestResult {
    let _guard = subscribe();
    let mut pair = ConnPair::builder()
        .with_server_endpoint_cfg(endpoint_config())
        .connect();
    let client_addr = pair.routes.as_basic().client_addr;
    let server_addr = pair.routes.as_basic().server_addr;
    let token = client_token(&pair, PathId::ZERO);

    info!("wrong token");
    let mut wrong: [u8; RESET_TOKEN_SIZE] = token[..].try_into()?;
    wrong[RESET_TOKEN_SIZE - 1] ^= 1;
    deliver_to_client(
        &mut pair,
        server_addr,
        client_addr,
        forged_reset(64, &wrong),
    );
    assert!(!pair.is_closed(Client));

    info!("right token, from another remote");
    let mut elsewhere = server_addr;
    elsewhere.set_port(server_addr.port() + 7);
    deliver_to_client(&mut pair, elsewhere, client_addr, forged_reset(64, &token));
    assert!(!pair.is_closed(Client));

    info!("right token, one byte short of a stateless reset");
    deliver_to_client(
        &mut pair,
        server_addr,
        client_addr,
        forged_reset(MIN_STATELESS_RESET_SIZE - 1, &token),
    );
    assert!(!pair.is_closed(Client));

    info!("the connection still works");
    pair.ping(Client);
    pair.drive();
    assert!(!pair.is_closed(Client));
    assert!(!pair.is_closed(Server));
    assert_eq!(lost_reason(&mut pair, Client), None);

    info!("right token, minimum size");
    let token = client_token(&pair, PathId::ZERO);
    deliver_to_client(
        &mut pair,
        server_addr,
        client_addr,
        forged_reset(MIN_STATELESS_RESET_SIZE, &token),
    );
    assert_eq!(lost_reason(&mut pair, Client), Some(ConnectionError::Reset));
    Ok(())
}

/// A stateless reset on any path closes the whole multipath connection
/// (draft-ietf-quic-multipath-21 §3.4), even while the path it started on is still open.
#[test]
fn reset_on_one_path_closes_the_whole_connection() -> TestResult {
    let _guard = subscribe();
    let (mut pair, _, _, second_client_addr, second_server_addr) = multipath_pair();
    let path_id = open_second_path(&mut pair, second_client_addr, second_server_addr);
    assert!(pair.paths(Client).contains(&PathId::ZERO));

    // Only the second path still carries traffic, so the reset can only arrive there.
    pair.routes =
        ManyToManyRouting::simple_symmetric([second_client_addr], [second_server_addr]).into();
    restart_server(&mut pair);
    info!("pinging the restarted server on {path_id}");
    pair.ping_path(Client, path_id)?;
    exchange(&mut pair);

    assert_eq!(lost_reason(&mut pair, Client), Some(ConnectionError::Reset));
    Ok(())
}

/// After the path the connection started on is abandoned and discarded, a reset on the path
/// that remains still closes the connection.
#[test]
fn reset_after_first_path_is_discarded() -> TestResult {
    let _guard = subscribe();
    let (mut pair, _, _, second_client_addr, second_server_addr) = multipath_pair();
    let path_id = open_second_path(&mut pair, second_client_addr, second_server_addr);

    pair.close_path(Client, PathId::ZERO, VarInt(0))?;
    pair.drive();
    while pair.poll(Client).is_some() {}
    while pair.poll(Server).is_some() {}
    assert_eq!(pair.paths(Client), vec![path_id]);

    restart_server(&mut pair);
    info!("pinging the restarted server on {path_id}");
    pair.ping_path(Client, path_id)?;
    exchange(&mut pair);

    assert_eq!(lost_reason(&mut pair, Client), Some(ConnectionError::Reset));
    Ok(())
}

/// The token of an abandoned path's retired CID must not close the connection
/// (RFC 9000 §10.3.1, draft-ietf-quic-multipath-21 §3.4.2); a token in use still does.
#[test]
fn reset_with_token_of_abandoned_path_is_ignored() -> TestResult {
    let _guard = subscribe();
    let (mut pair, client_addr, server_addr, second_client_addr, second_server_addr) =
        multipath_pair();
    let path_id = open_second_path(&mut pair, second_client_addr, second_server_addr);
    let abandoned_token = client_token(&pair, path_id);

    pair.close_path(Client, path_id, VarInt(0))?;
    pair.drive();
    while pair.poll(Client).is_some() {}
    while pair.poll(Server).is_some() {}
    assert_eq!(pair.paths(Client), vec![PathId::ZERO]);

    info!("token of the abandoned path");
    deliver_to_client(
        &mut pair,
        second_server_addr,
        second_client_addr,
        forged_reset(64, &abandoned_token),
    );
    assert!(!pair.is_closed(Client));

    info!("token of the open path");
    let token = client_token(&pair, PathId::ZERO);
    deliver_to_client(
        &mut pair,
        server_addr,
        client_addr,
        forged_reset(64, &token),
    );
    assert_eq!(lost_reason(&mut pair, Client), Some(ConnectionError::Reset));
    Ok(())
}
