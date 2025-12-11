/**
 * Geospatial Operations Examples
 * 
 * Demonstrates geospatial operations: GEOADD, GEODIST, GEORADIUS, GEOPOS
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runGeospatialExamples() {
  console.log('🌍 === GEOSPATIAL OPERATIONS EXAMPLES ===\n');

  try {
    // GEOADD
    await synap.geospatial.geoadd('locations', [
      { member: 'restaurant1', lon: -122.4194, lat: 37.7749 },
      { member: 'restaurant2', lon: -122.4094, lat: 37.7849 },
      { member: 'restaurant3', lon: -122.4294, lat: 37.7649 },
    ]);
    console.log('✅ GEOADD locations');

    // GEODIST
    const distance = await synap.geospatial.geodist(
      'locations',
      'restaurant1',
      'restaurant2',
      'km'
    );
    console.log('✅ GEODIST:', distance, 'km');

    // GEORADIUS
    const nearby = await synap.geospatial.georadius(
      'locations',
      37.7749,  // centerLat
      -122.4194,  // centerLon
      5,
      'km'
    );
    console.log('✅ GEORADIUS:', nearby);

    // GEOPOS
    const position = await synap.geospatial.geopos('locations', ['restaurant1']);
    console.log('✅ GEOPOS:', position);

    // STATS
    const geoStats = await synap.geospatial.stats();
    console.log('✅ Geospatial Stats:', geoStats);

    console.log('\n✅ Geospatial operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runGeospatialExamples().catch(console.error);

