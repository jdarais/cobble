FROM python:3.10-slim

RUN python -m pip install poetry

COPY target/release/cobl /usr/bin/cobl
COPY examples/python_poetry/workspace/ /repo/

WORKDIR /repo
RUN poetry lock
RUN cobl run poetry_env

