# Daily Hacker News LINE Chatbot

This is a LINE chatbot that sends daily hacker news to friends. The news source is obtained from https://www.daemonology.net/hn-daily/.

## Requirements

To run this project, you need to have the following:
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

## Daily Hacker News Chatbot
![QR Code](https://github.com/wisehuang/daily_hacker_news_line_chatbot/blob/main/623yruqr.png)

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

Please make sure to update tests as appropriate.

## License

This project is licensed under the MIT License. See the LICENSE file for more details.
