import { WebSocket } from 'ws';

const url = 'wss://midnight-preprod.blockfrost.io/api/v0/ws?project_id=nightpreprodDtPVSfdQxEC2ewlYn2skOPcMOarKA8A6';
const ws = new WebSocket(url, 'graphql-transport-ws');

ws.on('open', () => {
  console.log('connected');
  ws.send(JSON.stringify({ type: 'connection_init' }));
});

ws.on('message', (data) => {
  const msg = JSON.parse(data.toString());
  console.log('received:', JSON.stringify(msg, null, 2));
  if (msg.type === 'connection_ack') {
    ws.send(JSON.stringify({
      id: 'sub-1',
      type: 'subscribe',
      payload: {
        query: `subscription UnshieldedTransactions($address: UnshieldedAddress!, $transactionId: Int) {
          unshieldedTransactions(address: $address, transactionId: $transactionId) {
            ... on UnshieldedTransaction {
              type: __typename
              transaction {
                type: __typename
                id
                hash
                protocolVersion
                block {
                  timestamp
                }
                ... on RegularTransaction {
                  identifiers
                  fees {
                    paidFees
                    estimatedFees
                  }
                  transactionResult {
                    status
                    segments {
                      id
                      success
                    }
                  }
                }
              }
              createdUtxos {
                owner
                tokenType
                value
                outputIndex
                intentHash
                ctime
                registeredForDustGeneration
              }
              spentUtxos {
                owner
                tokenType
                value
                outputIndex
                intentHash
                ctime
                registeredForDustGeneration
              }
            }
            ... on UnshieldedTransactionsProgress {
              type: __typename
              highestTransactionId
            }
          }
        }`,
        variables: {
          address: "mn_addr_preprod1kn0rjs8whwg7jd4kr8dxda023k8wduky2qzalrhfxh60z2kqerjqp2nng5",
          transactionId: 0
        }
      }
    }));
  }
});

ws.on('error', (err) => console.error('WS Error:', err));
setTimeout(() => { ws.close(); process.exit(0); }, 5000);
