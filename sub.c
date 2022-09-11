// subtitle conversion

// Copyright (C) 2022 Daan Meijer
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <https://www.gnu.org/licenses/>.
#include <stos.h>
#include <stdlib.h>
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>

#include <stdio.h>

struct subtitle *get_subs(const char *url, size_t *n)
{
	const AVCodec *codec = NULL;
	AVFormatContext *fctx = NULL;

	if (avformat_open_input(&fctx, url, NULL, NULL) < 0)
		return NULL;
	if (avformat_find_stream_info(fctx, NULL) < 0)
		return NULL;
	if (fctx->nb_streams <= 0)
		return NULL;
	codec = avcodec_find_decoder(fctx->streams[0]->codecpar->codec_id);
	if (codec == NULL) {
		fprintf(stderr, "failed to find codec\n");
		return NULL;
	}
	
	AVPacket *pkt = NULL;
	pkt = av_packet_alloc();
	if (pkt == NULL) {
		fprintf(stderr, "failed to allocate packet\n");
		return NULL;
	}

	AVCodecContext *cctx = avcodec_alloc_context3(codec);
	if (cctx == NULL) {
		fprintf(stderr, "failed to allocate codec context\n");
		return NULL;
	}
	//TODO try without dictionary

	AVDictionary *opts = NULL;
	if (av_dict_set(&opts, "b", "2.5M", 0) < 0) {
		fprintf(stderr, "failed to set dictionary\n");
		return NULL;
	}
	if (avcodec_open2(cctx, codec, &opts) < 0) {
		fprintf(stderr, "could not open2 codec\n");
		return NULL;
	}

	int got;
	AVSubtitle sub;
	while (av_read_frame(fctx, pkt) == 0) {
		if (avcodec_decode_subtitle2(cctx, &sub, &got, pkt) < 0) {
			fprintf(stderr, "%s: %s\n", "conv",
					"failed to decode subtitle");
			return NULL;
		}
		for (size_t idx = 0; idx < sub.num_rects; ++idx) {
			fprintf(stdout, "type: %d text:%s\n",
					(int) sub.rects[idx]->type,
					sub.rects[idx]->ass);
		}
		if (got) {
			avsubtitle_free(&sub);
		}
		av_packet_unref(pkt);
	}
	avformat_close_input(&fctx);
	return NULL;
}

