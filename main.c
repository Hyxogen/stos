// main entrypoint stos

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
#include <stdio.h>
#include <errno.h>
#include <string.h>
#include "libavcodec/avcodec.h"
#include "libavformat/avformat.h"

int main(int argc, char *argv[])
{
	const AVCodec *codec = NULL;
	AVCodecContext *c = NULL;
	AVDictionary *opt = NULL;
	AVPacket *pkt = NULL;
	FILE *in = NULL;
	AVSubtitle sub;
	AVFormatContext *fctx = NULL;
	int ret, got;
	
	if (argc != 2) {
		fprintf(stderr, "usage: %s <in_file>\n", argv[0]);
		return EXIT_FAILURE;
	}
	pkt = av_packet_alloc();
	if (pkt == NULL) {
		fprintf(stderr, "failed to allocate avpacket\n");
		return EXIT_FAILURE;
	}
	codec = avcodec_find_decoder_by_name("srt");
	if (codec == NULL) {
		fprintf(stderr, "failed to find decoder\n");
		return EXIT_FAILURE;
	}
	c = avcodec_alloc_context3(codec);
	if (c == NULL) {
		fprintf(stderr, "could not open codec\n");
		return EXIT_FAILURE;
	}
	if (av_dict_set(&opt, "b", "2.5M", 0) < 0) {
		fprintf(stderr, "could not set dictionary\n");
		av_free(c);
		return EXIT_FAILURE;
	}
	if (avcodec_open2(c, codec, &opt) < 0) {
		fprintf(stderr, "could not open2 codec\n");
		av_free(c);
		return EXIT_FAILURE;
	}
	in = fopen(argv[1], "rb");
	if (in == NULL) {
		fprintf(stderr, "%s: %s\n", argv[1], strerror(errno));
		av_free(c);
		return EXIT_FAILURE;
	}
	ret = avformat_open_input(&fctx, argv[1], NULL, NULL);
	if (ret < 0) {
		fprintf(stderr, "%s: %s\n", argv[1], "failed to open format");
		return EXIT_FAILURE;
	}
	while (av_read_frame(fctx, pkt) == 0) {
		if (avcodec_decode_subtitle2(c, &sub, &got, pkt) < 0) {
			fprintf(stderr, "%s: %s\n", argv[1],
					"failed to decode subtitle");
			return EXIT_FAILURE;
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
	pkt->data = NULL;
	pkt->size = 0;
	fclose(in);
	avcodec_free_context(&c);
	av_packet_free(&pkt);
	avformat_close_input(&fctx);
	return EXIT_SUCCESS;
}

			
