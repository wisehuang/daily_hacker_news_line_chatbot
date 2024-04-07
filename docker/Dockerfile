# Use the official Rust image as Base
FROM rust:1.77.1-slim as builder

# Install OpenSSL
RUN apt update && apt install -y libssl-dev pkg-config make g++ perl && rustup target add x86_64-unknown-linux-gnu

# Create a new empty shell project
RUN USER=root cargo new --bin daily_hacker_news_line_chatbot
WORKDIR /daily_hacker_news_line_chatbot

# Copy our manifests into the project
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# Build this project, pulling in necessary dependencies
RUN rm src/*.rs

# Copy our source code into the project
COPY ./src ./src
COPY ./config.toml ./config.toml
COPY ./prompts.toml ./prompts.toml

# Build for release.
RUN cargo clean
RUN cargo build --release

# Our final base
FROM debian:trixie-slim

# Change to the directory containing the executable
WORKDIR /daily_hacker_news_line_chatbot

# Copy the build artifact from the builder stage and create a new user
RUN useradd daily_hacker_news_line_chatbot
COPY --from=builder /daily_hacker_news_line_chatbot/target/release/daily_hacker_news_line_chatbot ./

# Change to this new user
USER daily_hacker_news_line_chatbot

# Execute the file
CMD ["/daily_hacker_news_line_chatbot/daily_hacker_news_line_chatbot"]