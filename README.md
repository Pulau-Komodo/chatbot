# Chatbot

This is a fairly simple Discord bot for talking with GPT. It uses Serenity to interface with Discord and SQLx with SQLite to store conversations and usage information.

To start a conversation, the user mentions the bot at the start or end of a message. To continue a conversation, the user replies (with ping) to a previous GPT message from the bot. Users can reply to a message even if it was directed to another user, and even if the message was already replied to.

An alternative way of starting a conversation is pinging the bot while replying to a message not from the bot. This will submit the other message's contents as a query if the pinging message is otherwise blank, or add it in quotation marks to the end of the pinging message if it's not blank.

The bot tracks per-user GPT credit allowance, which regenerates constantly. Users use up this allowance as they interact with GPT, and cannot interact further while their allowance is below 0.