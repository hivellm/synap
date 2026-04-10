//! Mock-based tests for Geospatial operations (no running server required)

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;
    use synap_sdk::geospatial::{DistanceUnit, Location};

    #[tokio::test]
    async fn test_geospatial_geoadd() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.geoadd",
                "payload": {
                    "key": "places",
                    "nx": false,
                    "xx": false,
                    "ch": false
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"added": 2}}"#)
            .create_async()
            .await;

        let locations = vec![
            Location {
                lat: 40.7128,
                lon: -74.0060,
                member: "NYC".into(),
            },
            Location {
                lat: 34.0522,
                lon: -118.2437,
                member: "LA".into(),
            },
        ];
        let added = client
            .geospatial()
            .geoadd("places", locations, false, false, false)
            .await
            .unwrap();
        assert_eq!(added, 2);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_geoadd_nx() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.geoadd",
                "payload": {
                    "key": "places",
                    "nx": true,
                    "xx": false,
                    "ch": false
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"added": 1}}"#)
            .create_async()
            .await;

        let locations = vec![Location {
            lat: 51.5074,
            lon: -0.1278,
            member: "London".into(),
        }];
        let added = client
            .geospatial()
            .geoadd("places", locations, true, false, false)
            .await
            .unwrap();
        assert_eq!(added, 1);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_geoadd_invalid_lat() {
        let (client, _server) = setup_test_client().await;
        let locations = vec![Location {
            lat: 91.0,
            lon: 0.0,
            member: "invalid".into(),
        }];
        let result = client
            .geospatial()
            .geoadd("places", locations, false, false, false)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_geospatial_geoadd_invalid_lon() {
        let (client, _server) = setup_test_client().await;
        let locations = vec![Location {
            lat: 0.0,
            lon: 181.0,
            member: "invalid".into(),
        }];
        let result = client
            .geospatial()
            .geoadd("places", locations, false, false, false)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_geospatial_geodist() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.geodist",
                "payload": {
                    "key": "places",
                    "member1": "NYC",
                    "member2": "LA",
                    "unit": "km"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"distance": 3944.42}}"#)
            .create_async()
            .await;

        let dist = client
            .geospatial()
            .geodist("places", "NYC", "LA", DistanceUnit::Kilometers)
            .await
            .unwrap();
        assert_eq!(dist, Some(3944.42));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_geodist_missing_member() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.geodist",
                "payload": {
                    "key": "places",
                    "member1": "NYC",
                    "member2": "unknown",
                    "unit": "m"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        let dist = client
            .geospatial()
            .geodist("places", "NYC", "unknown", DistanceUnit::Meters)
            .await
            .unwrap();
        assert_eq!(dist, None);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_georadius() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.georadius",
                "payload": {
                    "key": "places",
                    "center_lat": 40.0,
                    "center_lon": -74.0,
                    "radius": 500.0,
                    "unit": "km",
                    "with_dist": true,
                    "with_coord": false
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"results": [{"member": "NYC", "distance": 80.5}]}}"#)
            .create_async()
            .await;

        let results = client
            .geospatial()
            .georadius(
                "places",
                40.0,
                -74.0,
                500.0,
                DistanceUnit::Kilometers,
                true,
                false,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].member, "NYC");
        assert_eq!(results[0].distance, Some(80.5));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_georadius_invalid_lat() {
        let (client, _server) = setup_test_client().await;
        let result = client
            .geospatial()
            .georadius(
                "places",
                91.0,
                0.0,
                100.0,
                DistanceUnit::Meters,
                false,
                false,
                None,
                None,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_geospatial_georadius_with_count_and_sort() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.georadius",
                "payload": {
                    "key": "places",
                    "count": 5,
                    "sort": "ASC"
                }
            })))
            .with_status(200)
            .with_body(
                r#"{"success": true, "payload": {"results": [{"member": "A"}, {"member": "B"}]}}"#,
            )
            .create_async()
            .await;

        let results = client
            .geospatial()
            .georadius(
                "places",
                0.0,
                0.0,
                1000.0,
                DistanceUnit::Kilometers,
                false,
                false,
                Some(5),
                Some("ASC"),
            )
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_georadiusbymember() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.georadiusbymember",
                "payload": {
                    "key": "places",
                    "member": "NYC",
                    "radius": 200.0,
                    "unit": "mi",
                    "with_dist": true,
                    "with_coord": true
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"results": [{"member": "NYC", "distance": 0.0, "coord": {"lat": 40.7128, "lon": -74.006}}]}}"#)
            .create_async()
            .await;

        let results = client
            .geospatial()
            .georadiusbymember(
                "places",
                "NYC",
                200.0,
                DistanceUnit::Miles,
                true,
                true,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].member, "NYC");
        assert!(results[0].coord.is_some());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_geopos() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.geopos",
                "payload": {
                    "key": "places",
                    "members": ["NYC", "unknown"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"coordinates": [{"lat": 40.7128, "lon": -74.006}, null]}}"#)
            .create_async()
            .await;

        let coords = client
            .geospatial()
            .geopos("places", &["NYC".into(), "unknown".into()])
            .await
            .unwrap();
        assert_eq!(coords.len(), 2);
        assert!(coords[0].is_some());
        assert!(coords[1].is_none());
        let nyc = coords[0].as_ref().unwrap();
        assert!((nyc.lat - 40.7128).abs() < 0.001);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_geohash() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.geohash",
                "payload": {
                    "key": "places",
                    "members": ["NYC"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"geohashes": ["dr5regw3pp"]}}"#)
            .create_async()
            .await;

        let hashes = client
            .geospatial()
            .geohash("places", &["NYC".into()])
            .await
            .unwrap();
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0], Some("dr5regw3pp".into()));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_geosearch_by_radius() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.geosearch",
                "payload": {
                    "key": "places",
                    "from_lonlat": [-74.0, 40.7],
                    "by_radius": [100.0, "km"],
                    "with_dist": true,
                    "with_coord": false,
                    "with_hash": false
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"results": [{"member": "NYC", "distance": 1.5}]}}"#)
            .create_async()
            .await;

        let results = client
            .geospatial()
            .geosearch(
                "places",
                None,
                Some((-74.0, 40.7)),
                Some((100.0, DistanceUnit::Kilometers)),
                None,
                true,
                false,
                false,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].member, "NYC");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_geosearch_by_box() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.geosearch",
                "payload": {
                    "key": "places",
                    "from_member": "NYC",
                    "by_box": [200.0, 200.0, "km"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"results": [{"member": "NYC"}, {"member": "Philly"}]}}"#)
            .create_async()
            .await;

        let results = client
            .geospatial()
            .geosearch(
                "places",
                Some("NYC"),
                None,
                None,
                Some((200.0, 200.0, DistanceUnit::Kilometers)),
                false,
                false,
                false,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_geospatial_geosearch_missing_from() {
        let (client, _server) = setup_test_client().await;
        let result = client
            .geospatial()
            .geosearch(
                "places",
                None,
                None,
                Some((100.0, DistanceUnit::Meters)),
                None,
                false,
                false,
                false,
                None,
                None,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_geospatial_geosearch_missing_by() {
        let (client, _server) = setup_test_client().await;
        let result = client
            .geospatial()
            .geosearch(
                "places",
                Some("NYC"),
                None,
                None,
                None,
                false,
                false,
                false,
                None,
                None,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_geospatial_stats() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "geospatial.stats"
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"total_keys": 2, "total_locations": 50, "geoadd_count": 30, "geodist_count": 10, "georadius_count": 5, "geopos_count": 3, "geohash_count": 2}}"#)
            .create_async()
            .await;

        let stats = client.geospatial().stats().await.unwrap();
        assert_eq!(stats.total_keys, 2);
        assert_eq!(stats.total_locations, 50);
        assert_eq!(stats.geoadd_count, 30);

        mock.assert_async().await;
    }
}
