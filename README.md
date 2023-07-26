# Chatbot

This is a fairly simple Discord bot for talking with ChatGPT. It uses Serenity to interface with Discord and SQLx with SQLite to store conversations and usage information.

To start a conversation, the user mentions the bot at the start or end of a message. To continue a conversation, reply (with ping) to a previous ChatGPT message from the bot. Users can reply to a message even if it was directed to another user, and even if the message was already replied to.

Alternatively, a user can reply to a message and mention the bot (with no other text) to start a conversation with the text of that message.

The bot tracks per-user ChatGPT credit allowance, which regenerates constantly. Users use up this allowance as they interact with ChatGPT, and cannot interact further while their allowance is below 0.