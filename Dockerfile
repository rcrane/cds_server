FROM rust:1.33.0-slim

ADD . /tmp/cds_server

RUN cd /tmp/cds_server && \
    cargo build --release && \
    cp target/release/cds_server /usr/local/bin && \
    cp cds_server.json /etc/cds_server.json && \
    rm -r /tmp/cds_server

ENV CDS_PORT 8080

ENV RUST_LOG debug

CMD cds_server -c /etc/cds_server.json -p $CDS_PORT