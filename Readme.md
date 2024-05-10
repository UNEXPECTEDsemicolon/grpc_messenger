# gRPC Messanger

## Components:

### 1. Backend messenger server
Implemented in Rust using [tonic](https://github.com/hyperium/tonic) gRPC framework

**How to run**: `./run_messenger_server.sh`

### 2. Console messenger client
Implemented in Rust using [tonic](https://github.com/hyperium/tonic) gRPC framework

**How to run**: `./run_console_client.sh`


### 3. Web-client:
#### Frontend:
HTML + JavaScript + [gRPC Web](https://github.com/grpc/grpc-web) (packed by [webpack](https://webpack.js.org/))

#### Backend:
[Nginx](https://www.nginx.com/) (static routing) + [Envoy](https://www.envoyproxy.io/) (http2 requests from gRPC Web into gRPC for Rust backend)

**How to run**: `./run_web_client.sh`

## Notes:

All the components are built using only docker-compose. No extra requirement is needed. Feel free to edit ports in `.env` file.

## Video: [watch](./Video.mp4)