const grpc = require('@grpc/grpc-js');
const protoLoader = require('@grpc/proto-loader');
const path = require('path');

const PROTO_PATH = path.join(__dirname, '../proto/hashservice.proto');
const packageDefinition = protoLoader.loadSync(PROTO_PATH, {
    keepCase: true,
    longs: String,
    enums: String,
    defaults: true,
    oneofs: true
});
const hashservice = grpc.loadPackageDefinition(packageDefinition).hashservice;

function createClient(address = 'localhost:50051') {
    return new hashservice.HashLoader(address, grpc.credentials.createInsecure());
}

function hexToUint64String(hex) {
    // accepts string like 'a7cf...' (without 0x) and returns decimal string for uint64
    return BigInt(`0x${hex}`).toString();
}

function rpcCall(client, method, payload) {
    return new Promise((resolve, reject) => {
        client[method](payload, (err, res) => {
            if (err) reject(err);
            else resolve(res);
        });
    });
}

async function getString(client, hashHex, hashtableType = 'game') {
    const payload = {
        hash: hexToUint64String(hashHex),
        hashtable_type: hashtableType
    };
    return rpcCall(client, 'GetString', payload);
}

async function addHash(client, stringValue, hashtableType = 'game') {
    const payload = {
        string: stringValue,
        hashtable_type: hashtableType
    };
    return rpcCall(client, 'AddHash', payload);
}

async function unloadHashes(client) {
    return rpcCall(client, 'UnloadHashes', {});
}

async function main() {
    const client = createClient();
    const exampleHash = '46226a0b';
    const stringToHash = 'Characters/Aatrox/CAC/Aatrox_Skin02';
    const hashtableType = 'bin';

    try {
        console.log(`Getting string for hash ${exampleHash}...`);
        let resp = await getString(client, exampleHash, hashtableType);
        console.log('GetString response:', resp);

        console.log('\nAdding new hash entry...');
        const addResp = await addHash(client, stringToHash, 'bin');
        console.log('AddHash response:', addResp);

        console.log('\nVerifying insertion by fetching again...');
        resp = await getString(client, exampleHash, hashtableType);
        console.log('GetString response after add:', resp);

        // To unload hashes (uncomment if needed)
        // const unloadResp = await unloadHashes(client);
        // console.log('UnloadHashes response:', unloadResp);
    } catch (err) {
        console.error('Error:', err);
        process.exitCode = 1;
    }
}

main();

