from slack_sdk import WebClient
from slack_sdk.errors import SlackApiError
import os
from dotenv import load_dotenv


def pub_slack_message():
    load_dotenv()

    # print(os.getenv("SLACK_API_TOKEN"))
    client = WebClient(token=os.getenv("SLACK_API_TOKEN"))

    channel = "#general"
    message = "Hello from Python using a Slack bot!"

    try:
        response = client.chat_postMessage(channel=channel, text=message)
        print("Message posted successfully:", response)

    except SlackApiError as e:
        print("Error posting message to Slack:", e)


if __name__ == "__main__":
    pub_slack_message()
