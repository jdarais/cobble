FROM python:3.12-slim

COPY target/release/cobl /usr/bin/cobl
COPY test_repos/python_poetry/ /repo/

WORKDIR /repo

