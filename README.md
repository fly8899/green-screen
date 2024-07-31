# Local Green Screen Server

Start the server with ```cargo run``` and bind your client to <b>127.0.0.1:8080</b>. 

- The first image the camera sees will be used as background.
- You can change the FilterType (Color) by changing the Enum before starting the server.
- Your camera might behave different.

You will receive tcp payloads which contain the following:

```
{
    "width": 1280,
    "height": 720,
    "encoding-order": "RGBA",
    "image": [u8,u8,u8,u8]
}
```

