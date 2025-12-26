#!/usr/bin/env python3
import asyncio
import websockets
import json
import sys

async def chat_client(username, token):
    uri = "ws://127.0.0.1:8080"
    
    try:
        async with websockets.connect(uri) as websocket:
            print(f"Connected to server as {username}")
            
            # Send authentication message
            auth_msg = json.dumps({"token": token})
            await websocket.send(auth_msg)
            print(f"Sent auth token: {token}")
            
            # Wait for auth response
            auth_response = await websocket.recv()
            auth_data = json.loads(auth_response)
            
            if auth_data.get("type") == "auth_failed":
                print(f"Authentication failed: {auth_data.get('message')}")
                return
            
            print(f"Authentication successful!")
            
            # Create tasks for sending and receiving
            async def receive_messages():
                try:
                    async for message in websocket:
                        data = json.loads(message)
                        if data.get("from") == "SYSTEM":
                            print(f"\n[SYSTEM] {data.get('content')}")
                        else:
                            from_user = data.get("from")
                            to_user = data.get("to")
                            content = data.get("content")
                            
                            if to_user:
                                print(f"\n[DM from {from_user}] {content}")
                            else:
                                print(f"\n[{from_user}] {content}")
                        print("> ", end="", flush=True)
                except websockets.exceptions.ConnectionClosed:
                    print("\nConnection closed by server")
            
            async def send_messages():
                try:
                    while True:
                        # Read input from user
                        message = await asyncio.get_event_loop().run_in_executor(
                            None, input, "> "
                        )
                        
                        if message.lower() == "/quit":
                            break
                        
                        # Check if it's a DM (format: @username message)
                        if message.startswith("@"):
                            parts = message.split(" ", 1)
                            if len(parts) == 2:
                                target = parts[0][1:]  # Remove @
                                content = parts[1]
                                msg = json.dumps({
                                    "to": target,
                                    "content": content
                                })
                            else:
                                print("Invalid DM format. Use: @username message")
                                continue
                        else:
                            # Broadcast message
                            msg = json.dumps({
                                "to": None,
                                "content": message
                            })
                        
                        await websocket.send(msg)
                except asyncio.CancelledError:
                    pass
            
            # Run both tasks concurrently
            receive_task = asyncio.create_task(receive_messages())
            send_task = asyncio.create_task(send_messages())
            
            # Wait for send task to complete (user quits)
            await send_task
            receive_task.cancel()
            
            print("Disconnecting...")
    
    except websockets.exceptions.WebSocketException as e:
        print(f"WebSocket error: {e}")
    except Exception as e:
        print(f"Error: {e}")

def main():
    if len(sys.argv) < 2:
        print("Usage: python client.py <username>")
        print("Available usernames: alice, bob, charlie")
        sys.exit(1)
    
    username = sys.argv[1].lower()
    
    # Map username to token
    tokens = {
        "alice": "token-alice",
        "bob": "token-bob",
        "charlie": "token-charlie"
    }
    
    if username not in tokens:
        print(f"Unknown username: {username}")
        print("Available usernames: alice, bob, charlie")
        sys.exit(1)
    
    token = tokens[username]
    
    print(f"Starting chat client for {username}")
    print("Commands:")
    print("  - Type a message and press Enter to broadcast")
    print("  - Type @username message to send a DM")
    print("  - Type /quit to exit")
    print()
    
    asyncio.run(chat_client(username, token))

if __name__ == "__main__":
    main()