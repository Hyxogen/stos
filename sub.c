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
#include <stdio.h>
#include <stdarg.h>
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>


#ifndef ERROR_BUF_SIZE
# define ERROR_BUF_SIZE 1024
#endif
#if ERROR_BUF_SIZE <= 0
# error "ERROR_BUF_SIZE must be a positive integer"
#endif
static char error[ERROR_BUF_SIZE];

const char *stos_get_error(void) 
{
	return error;
}

static int stos_write_error(const char *restrict fmt, ...)
{
	int ret;
	
	va_list args;
	va_start(args, fmt);
	ret = vsnprintf(error, ERROR_BUF_SIZE, fmt, args);
	va_end(args);
	if (ret < 0)
		abort();
	return ret;
}

int get_file_info(struct file_info *info, const char *url)
{
	info->fctx = NULL;
	if (avformat_open_input(&info->fctx, url, NULL, NULL) < 0) {
		stos_write_error("%s: failed to open for input", url);
		goto error;
	}
	if (avformat_find_stream_info(info->fctx, NULL) < 0) {
		stos_write_error("%s: failed to retrieve stream info", url);
		goto error;
	}
	return 0;
error:
	return -1;
}

void del_file_info(struct file_info *info) 
{
	if (info == NULL)
		return;
	avformat_close_input(&info->fctx);
}
/*
  https://ffmpeg.org/doxygen/trunk/codec__id_8h_source.html
*/
static int stream_is_subtitle(const AVStream *stream) 
{
	enum AVCodecID id = stream->codecpar->codec_id;
	return id == AV_CODEC_ID_DVD_SUBTITLE ||
		id == AV_CODEC_ID_DVB_SUBTITLE ||
		id == AV_CODEC_ID_TEXT ||
		id == AV_CODEC_ID_XSUB ||
		id == AV_CODEC_ID_SSA ||
		id == AV_CODEC_ID_MOV_TEXT ||
		id == AV_CODEC_ID_HDMV_PGS_SUBTITLE ||
		id == AV_CODEC_ID_DVB_TELETEXT ||
		id == AV_CODEC_ID_SRT ||
		id == AV_CODEC_ID_MICRODVD ||
		id == AV_CODEC_ID_EIA_608 ||
		id == AV_CODEC_ID_JACOSUB ||
		id == AV_CODEC_ID_SAMI ||
		id == AV_CODEC_ID_REALTEXT ||
		id == AV_CODEC_ID_STL ||
		id == AV_CODEC_ID_SUBVIEWER1 ||
		id == AV_CODEC_ID_SUBVIEWER ||
		id == AV_CODEC_ID_SUBRIP ||
		id == AV_CODEC_ID_WEBVTT ||
		id == AV_CODEC_ID_MPL2 ||
		id == AV_CODEC_ID_VPLAYER ||
		id == AV_CODEC_ID_PJS ||
		id == AV_CODEC_ID_ASS ||
		id == AV_CODEC_ID_HDMV_TEXT_SUBTITLE ||
		id == AV_CODEC_ID_TTML ||
		id == AV_CODEC_ID_ARIB_CAPTION;
}

static AVStream* find_first_sub_stream(const struct file_info *info)
{
	for (size_t idx = 0; idx < info->fctx->nb_streams; ++idx) {
		if (stream_is_subtitle(info->fctx->streams[idx]))
			return info->fctx->streams[idx];
	}
	return NULL;
}

//https://ffmpeg.org/doxygen/trunk/transcoding_8c-example.html#a24
struct subtitle* get_subs(const struct file_info *info, int stream_idx, size_t *n)
{
	const AVCodec *codec = NULL;
	AVStream *stream = NULL;

	if (stream_idx < 0) {
		stream = find_first_sub_stream(info);
	} else if ((unsigned int) stream_idx < info->fctx->nb_streams) {
		stream = info->fctx->streams[stream_idx];
	}

	if (stream == NULL)
		goto cleanup;

	codec = avcodec_find_decoder(stream->codecpar->codec_id);
	if (codec == NULL) {
		stos_write_error("failed to find codec");
		goto cleanup;
	}
	
	AVPacket *pkt = NULL;
	pkt = av_packet_alloc();
	if (pkt == NULL) {
		stos_write_error("failed to allocate packet");
		goto cleanup;
	}

	AVCodecContext *cctx = avcodec_alloc_context3(codec);
	if (cctx == NULL) {
		stos_write_error("failed to allocate codec context");
		goto cleanup;
	}
	//TODO try without dictionary

	AVDictionary *opts = NULL;
	if (av_dict_set(&opts, "", "", 0) < 0) {
		stos_write_error("failed to set dictionary");
		goto cleanup;
	}
	if (avcodec_open2(cctx, codec, &opts) < 0) {
		stos_write_error("could not open2 codec");
		goto cleanup;
	}

	int got;
	AVSubtitle sub;
	while (av_read_frame(info->fctx, pkt) == 0) {
		if (avcodec_decode_subtitle2(cctx, &sub, &got, pkt) < 0) {
			stos_write_error("failed to decode subtitle");
			goto cleanup;
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
cleanup:
	if (cctx != NULL)
		avcodec_free_context(&cctx);
	if (pkt != NULL)
		av_packet_free(&pkt);
	if (opts != NULL)
		av_dict_free(&opts);
	return NULL;
}

