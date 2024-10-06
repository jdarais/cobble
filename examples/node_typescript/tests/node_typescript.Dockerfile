FROM node:22-slim

COPY target/release/cobl /usr/bin/cobl
COPY examples/node_typescript/workspace/ /repo/

WORKDIR /repo
RUN cobl run '*/npm_env'
