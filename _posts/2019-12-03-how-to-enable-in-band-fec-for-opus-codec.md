---
title: How to enable in-band FEC for Opus codec
published: true
permalink: "/how-to-enable-in-band-fec-for-opus-codec/"
share-img: /img/How-to-Test-Packet-Loss-on-Windows.png
share-description: "Minimal steps to enable in-band FEC for OPUS codec"
tags: [udp, opus, fec, rtcp, rtp]
readtime: true
---

The UDP network protocol does not support packet retransmission or acknowledgment out of the box; applications should handle it.
In audio/video transmission domain the problem is often solved by using [forward error correction](https://en.wikipedia.org/wiki/Forward_error_correction){:target="_blank"} technique.
The idea is to encode messages in a redundant way so that if a message is lost or corrupted, a receiver could detect errors and often correct these errors without retransmission.
<!--This redundant data is called FEC.-->

<!--![Packet loss image](/img/How-to-Test-Packet-Loss-on-Windows.png)-->

In general, it is up to a developer to decide how much redundant FEC data to encode at the sender side and decode it at the receiver.
Sometimes, it might be intricate, especially for newcomers.
Fortunately, OPUS codec developers made our life easier by implementing in-band FEC support.

To configure in-band FEC in OPUS codec, a user has to set the following configuration:

* Packet time (`ptime`) has to be not less than 10 ms, otherwise OPUS works in the [CELT](https://en.wikipedia.org/wiki/CELT){:target="_blank"} mode and not in [SILK](https://en.wikipedia.org/wiki/SILK){:target="_blank"}.
* The bitrate should be *slightly* higher.
For example, if the sample rate is 8kHz, then the bitrate should be from 12 kbps to 24 kbps.
The encoder needs a higher bitrate to have enough room for the LBRR frames containing FEC data.
* Do not use bitrates higher than 24 kbps, otherwise OPUS switches automatically to the CELT mode.
* FEC must be enabled via `OPUS_SET_INBAND_FEC(TRUE)`.
* Configure the encoder to expect packet loss percentage by setting `OPUS_SET_PACKET_LOSS_PERC(percentage)`.

Here is an example of configuring the OPUS encoder.

```c
opus_encoder_ctl(encoder, OPUS_SET_INBAND_FEC(TRUE));
opus_encoder_ctl(encoder,OPUS_SET_PACKET_LOSS_PERC(opus_packet_loss));
```

I configure `opus_packet_loss` via config file but it may be configured in realtime.
For example, you can update OPUS according to [RTCP](https://en.wikipedia.org/wiki/RTP_Control_Protocol){:target="_blank"}
statistics.

Now, after the encoder is ready, let's configure the decoder.
First of all, you need to know if a packet is lost.
The most natural way to know it is to check the [sequence number](https://en.wikipedia.org/wiki/Real-time_Transport_Protocol#Packet_header){:target="_blank"} of RTP packets.
If the previous packet got lost, then you should decode the current packet twice:
1. With FEC turned on to reproduce the lost packet
1. With FEC turned off to decode the current packet

The following snippet is an example of decoder's configuration:
```c
/* Decode the lost packet */
opus_decoder_ctl(decoder, OPUS_GET_LAST_PACKET_DURATION(frame_size));
opus_decode(	decoder,
		buffer, /* buffer to decode */,
		length, /* number of bytes in buffer */
		sampv,  /* output buffer */
		frame_size,
		1);   /* in-band FEC is turned on */
play_buffer(buffer);
/*Decode the current packet*/
opus_decode(	decoder,
		buffer, /* buffer to decode */,
		length, /* number of bytes in buffer */
		sampv,  /* output buffer */
		frame_size,
		0);   /* in-band FEC is turned off */
play_buffer(buffer);
```

## Summary ##

We learned how to configure in-band FEC for Opus codec, but the last thing to talk about is the pros and cons of using it!

### Pros: ###

* No need to develop an in-house mechanism of encoding FEC data.
* Most third parties like SIP servers (Asterisk) or WebRTC support Opus in-band FEC out of the box.
* FEC increases the audio quality when there is packet loss or corruption.

### Cons: ###

* Tricky to configure.
* There is no obvious way to see if traffic carries in-band FEC except generating packet loss and listening to
an audio sample. I used a sine wave generated as follows:
```plain
$ ffmpeg -f lavfi \
           -i "sine=frequency=1000:sample_rate=8000:duration=5" \
           output.wav
```
* Since an OPUS packet contains information only about the previous packet, in-band FEC can replicate only a single packet loss.
However, packets often get lost in a burst.
* Enabling FEC implicitly increases bitrate and bandwidth.
