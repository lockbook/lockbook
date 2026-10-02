# Chat

Lockbook can talk with an AI assistant about your notes. The assistant runs wherever you choose: a provider you have an account with, a server of your own on your network, or a model on the device itself. Lockbook's servers are never part of the conversation. Chats are files like any other, named `.chat`, so they sync, share, and search like notes.

## Setting up

Create a chat from the new-file menu. The first chat walks you through choosing a provider and pasting its key. The key is stored in your vault, encrypted like a note, under the hidden `.agent` folder. Add more providers from the model picker in the composer at any time, and pin the models you use so they stay at the top.

A server of your own, such as Ollama, LM Studio, or llama.cpp on a machine on your network, is added the same way with its address and no key. Models on such a server are often small; the assistant works harder for them and tells them when a note is long.

On a Mac or an iPhone with Apple Intelligence, the device's own model is offered first and needs nothing at all. It is kept to reading: it searches, lists, and reads notes and answers from them, but cannot edit, follows no `AGENTS.md`, and is shown no pictures. Ask it about one note at a time.

## What the assistant can do

The assistant works in the folder the chat lives in. It can search, list, and read the notes there, and it can edit, create, move, and delete them. Every call it makes shows in the chat as a row you can open, and edits show as word-level differences. It asks nobody before acting; scope is the only permission. Widen or narrow the folder from the composer's folder chip, and leave a chat in a folder of its own when you want it to see nothing else.

A note you mention in a message is read when you send it, and the assistant keeps that reading. Pictures are shown to models that take them, PDFs to those that read them, and a recording is read as its transcript, written once beside it.

An `AGENTS.md` note in a folder holds your standing instructions for work there: how a calendar is kept, what to do on a check-in. The assistant reads every `AGENTS.md` from the root down to the chat's folder, the deeper one having the later word.

Providers that search the web do so with their own tools. The others can always fetch a page you link; to let them search, add a search engine file under `.agent/search`: `{"kind": "brave", "api_key": "..."}` or `{"kind": "searxng", "base_url": "http://..."}`.

## Talking by voice

On a provider with a voice model, the phone button in the composer opens a spoken conversation. Speak, and the assistant answers aloud; speak over it and it stops where you cut in. What you said and what you heard of each reply are written into the chat as they settle, so a conversation reads afterward as it went. On a desktop without echo cancellation, wear headphones, or the reply is heard as you.

## What leaves your device

Your messages, the notes the assistant reads on your behalf, and the pictures or files it is shown go to the provider you chose, under that provider's terms; with a server of your own or the device's model, they go there and nowhere else. Two things reach further: a web search goes to the search engine you set up and a fetch to the site, and a recording you mention is transcribed once by the first of your providers that transcribes. Lockbook's servers never see any of it, and never see your keys. The chat shows where messages go before you send the first one.

## Settings

Each chat remembers, per person, the model and the folders it works in. The vault's `.agent/default.json` names the provider a new chat starts with, and `.agent/providers/<name>.json` holds each provider's address, model, and key. These are ordinary files: edit them, sync them, and they follow you.
