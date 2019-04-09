FROM rust:1.33.0-slim

# Copy source code of the cds server into the build container
ADD . /tmp/cds_server

# Build code and copy into container system paths
RUN cd /tmp/cds_server && \
    # build it
    cargo build --release && \
    # copy the built binary into the system
    cp target/release/cds_server /usr/local/bin && \
    # copy the server's config file into config directory
    cp cds_server.json /etc/cds_server.json && \
    # remove the source code from the build container
    rm -r /tmp/cds_server

# Set default port the server will listen on
ENV CDS_PORT 8080

# Activate debug output by default to help debugging issues
ENV RUST_LOG debug

# Start the cds server by default with its configuration file read
# from /etc/cds_server.json and its port read from environment
# variable CDS_PORT
CMD cds_server -c /etc/cds_server.json -p $CDS_PORT