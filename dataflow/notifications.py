from slack_sdk import WebClient
from slack_sdk.errors import SlackApiError
import os
from dotenv import load_dotenv


def pub_slack_message(message="test", token=None, channel="#general"):
    client: WebClient

    if token is None:
        load_dotenv()
        client = WebClient(token=os.getenv("SLACK_API_TOKEN"))
    else:
        client = WebClient(token=token)

    try:
        response = client.chat_postMessage(channel=channel, text=message)
        print("Message posted successfully:", response)

    except SlackApiError as e:
        print("Error posting message to Slack:", e)


if __name__ == "__main__":
    pub_slack_message()
