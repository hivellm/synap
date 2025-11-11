//! Geospatial S2S Integration Tests
//!
//! These tests require a running Synap server.
//! Run with: SYNAP_URL=http://localhost:15500 cargo test --test geospatial_s2s_test

mod common;

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_geospatial_geoadd() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:geospatial:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let locations = vec![
        synap_sdk::geospatial::Location {
            lat: 37.7749,
            lon: -122.4194,
            member: "San Francisco".to_string(),
        },
        synap_sdk::geospatial::Location {
            lat: 40.7128,
            lon: -74.0060,
            member: "New York".to_string(),
        },
    ];

    let added = client
        .geospatial()
        .geoadd(&key, locations, false, false, false)
        .await
        .unwrap();
    assert!(added > 0);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_geospatial_geodist() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:geospatial:dist:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let locations = vec![
        synap_sdk::geospatial::Location {
            lat: 37.7749,
            lon: -122.4194,
            member: "San Francisco".to_string(),
        },
        synap_sdk::geospatial::Location {
            lat: 40.7128,
            lon: -74.0060,
            member: "New York".to_string(),
        },
    ];

    client
        .geospatial()
        .geoadd(&key, locations, false, false, false)
        .await
        .unwrap();

    let distance = client
        .geospatial()
        .geodist(
            &key,
            "San Francisco",
            "New York",
            synap_sdk::geospatial::DistanceUnit::Kilometers,
        )
        .await
        .unwrap();

    assert!(distance.is_some());
    assert!(distance.unwrap() > 0.0);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_geospatial_georadius() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:geospatial:radius:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let locations = vec![
        synap_sdk::geospatial::Location {
            lat: 37.7749,
            lon: -122.4194,
            member: "San Francisco".to_string(),
        },
        synap_sdk::geospatial::Location {
            lat: 37.8044,
            lon: -122.2711,
            member: "Oakland".to_string(),
        },
    ];

    client
        .geospatial()
        .geoadd(&key, locations, false, false, false)
        .await
        .unwrap();

    let results = client
        .geospatial()
        .georadius(
            &key,
            37.7749,
            -122.4194,
            50.0,
            synap_sdk::geospatial::DistanceUnit::Kilometers,
            true,
            false,
            None,
            None,
        )
        .await
        .unwrap();

    assert!(!results.is_empty());
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_geospatial_geopos() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:geospatial:geopos:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let locations = vec![synap_sdk::geospatial::Location {
        lat: 37.7749,
        lon: -122.4194,
        member: "San Francisco".to_string(),
    }];

    client
        .geospatial()
        .geoadd(&key, locations, false, false, false)
        .await
        .unwrap();

    let coords = client
        .geospatial()
        .geopos(&key, &["San Francisco".to_string()])
        .await
        .unwrap();

    assert_eq!(coords.len(), 1);
    assert!(coords[0].is_some());
    let coord = coords[0].as_ref().unwrap();
    assert!((coord.lat - 37.7749).abs() < 0.01);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_geospatial_geohash() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:geospatial:geohash:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let locations = vec![synap_sdk::geospatial::Location {
        lat: 37.7749,
        lon: -122.4194,
        member: "San Francisco".to_string(),
    }];

    client
        .geospatial()
        .geoadd(&key, locations, false, false, false)
        .await
        .unwrap();

    let geohashes = client
        .geospatial()
        .geohash(&key, &["San Francisco".to_string()])
        .await
        .unwrap();

    assert_eq!(geohashes.len(), 1);
    assert!(geohashes[0].is_some());
    assert_eq!(geohashes[0].as_ref().unwrap().len(), 11);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_geospatial_geosearch_from_member_by_radius() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:geospatial:geosearch:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let locations = vec![
        synap_sdk::geospatial::Location {
            lat: 37.7749,
            lon: -122.4194,
            member: "San Francisco".to_string(),
        },
        synap_sdk::geospatial::Location {
            lat: 37.8044,
            lon: -122.2711,
            member: "Oakland".to_string(),
        },
        synap_sdk::geospatial::Location {
            lat: 40.7128,
            lon: -74.0060,
            member: "New York".to_string(),
        },
    ];

    client
        .geospatial()
        .geoadd(&key, locations, false, false, false)
        .await
        .unwrap();

    let results = client
        .geospatial()
        .geosearch(
            &key,
            Some("San Francisco"),
            None,
            Some((50.0, synap_sdk::geospatial::DistanceUnit::Kilometers)),
            None,
            true,
            false,
            false,
            None,
            None,
        )
        .await
        .unwrap();

    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.member == "San Francisco"));
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_geospatial_geosearch_from_lonlat_by_radius() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:geospatial:geosearch:lonlat:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let locations = vec![
        synap_sdk::geospatial::Location {
            lat: 37.7749,
            lon: -122.4194,
            member: "San Francisco".to_string(),
        },
        synap_sdk::geospatial::Location {
            lat: 37.8044,
            lon: -122.2711,
            member: "Oakland".to_string(),
        },
    ];

    client
        .geospatial()
        .geoadd(&key, locations, false, false, false)
        .await
        .unwrap();

    let results = client
        .geospatial()
        .geosearch(
            &key,
            None,
            Some((-122.4194, 37.7749)),
            Some((50.0, synap_sdk::geospatial::DistanceUnit::Kilometers)),
            None,
            true,
            true,
            false,
            None,
            None,
        )
        .await
        .unwrap();

    assert!(!results.is_empty());
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_geospatial_geosearch_by_box() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:geospatial:geosearch:box:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let locations = vec![
        synap_sdk::geospatial::Location {
            lat: 37.7749,
            lon: -122.4194,
            member: "San Francisco".to_string(),
        },
        synap_sdk::geospatial::Location {
            lat: 37.8044,
            lon: -122.2711,
            member: "Oakland".to_string(),
        },
    ];

    client
        .geospatial()
        .geoadd(&key, locations, false, false, false)
        .await
        .unwrap();

    let results = client
        .geospatial()
        .geosearch(
            &key,
            Some("San Francisco"),
            None,
            None,
            Some((
                100000.0,
                100000.0,
                synap_sdk::geospatial::DistanceUnit::Meters,
            )),
            false,
            true,
            false,
            None,
            None,
        )
        .await
        .unwrap();

    assert!(!results.is_empty());
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_geospatial_stats() {
    let client = common::setup_s2s_client();

    let stats = client.geospatial().stats().await.unwrap();

    // Stats should be non-negative (usize is always >= 0)
    let _ = stats.total_keys;
    let _ = stats.total_locations;
}
