# Daily Hacker News LINE Chatbot

This is a LINE chatbot that sends daily hacker news to friends. The news source is obtained from https://www.daemonology.net/hn-daily/.

This project is a Rust-based application that utilizes various libraries and frameworks such as reqwest, serde, warp, and tokio. It is designed to handle various tasks including reading RSS feeds, handling HTTP requests, and interacting with the OpenAI GPT-4 model, and Kagi Universal Summary API.

## Requirements

To run this project, you need to have the following:
* [Rust](https://www.rust-lang.org/tools/install)
* A LINE developer account
* A channel access token and channel secret for your LINE channel
* An OpenAI API token
* A Kagi API token
* A server to host the webhook endpoint

## Installation

1. Clone this repository to your local machine.
2. Install Rust and its dependencies. For more information, refer to the official Rust documentation.
3. Set up your config.toml file 
4. Build the project by running 

```bash
cargo build --release
```

5. Run the project with 
```bash
./target/release/daily_hacker_news_line_chatbot
```
6. The service will start listening to port 3030.

## Docker Usage

This application can be built and run using Docker. We use `podman` as a drop-in replacement for Docker. If you have Docker installed, you can replace `podman` with `docker` in the commands below.

To build the Docker image, navigate to the project root directory and run the following command:

```bash
podman build -f docker/Dockerfile -t daily_hacker_news_line_chatbot .
```

To run the Docker container and expose port 3030, use the following command:
```bash
podman run -p 3030:3030 localhost/daily_hacker_news_line_chatbot:latest
```
Remember to replace `podman` with `docker` if you are using Docker.

## Daily Hacker News Chatbot
![QR Code](https://github.com/wisehuang/daily_hacker_news_line_chatbot/blob/main/623yruqr.png)

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

Please make sure to update tests as appropriate.

## License

This project is licensed under the MIT License. See the LICENSE file for more details.
