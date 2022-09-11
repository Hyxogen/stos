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
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
#include <stos.h>
#include <stos_util.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdarg.h>
#include <string.h>
#include <ctype.h>
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

void del_sub(struct subtitle *sub) 
{
	if (sub == NULL)
		return;
	for (size_t idx = 0; idx < sub->num_text; ++idx) {
		free(sub->text[idx]);
	}
	free(sub->text);
}

void del_subs(struct subtitle *subs, size_t n)
{
	for (size_t idx = 0; idx < n; ++idx) {
		del_sub(subs + idx);
	}
	free(subs);
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

static char *nstrchr(const char *str, char ch, size_t n)
{
	while (n > 0 && (str = strchr(str, ch)) != NULL) {
		n--;
		str++;
	}
	return (char *) str;
}

static int parse_ass(struct subtitle *dst, size_t idx, const char *event)
{
	size_t i = 0;
	size_t j = 0;
	size_t brackets = 0;
	size_t len = 0;
	
	event = nstrchr(event, ',', 8);
	if (event == NULL || *event == '\0') {
		stos_write_error("invalid format");
		return -1;
	}
	
	len = strlen(event);
	dst->text[idx] = malloc(len + 1);
	if (dst->text[idx] == NULL) {
		stos_write_error("out of memory");
		return -1;
	}
	while (i < len) {
		if (event[i] == '{') {
			dst->styled = 1;
			brackets += 1;
		} else if (event[i] == '}' && brackets != 0) {
			brackets -= 1;
		} else if (event[i] == '\\' && i + 1 < len
			   && tolower(event[i]) == 'n') {
			dst->text[idx][j] = '\n';
			j += 1;
		} else if (brackets == 0) {
			dst->text[idx][j] = event[i];
			j += 1;
		}
		i += 1;
	}
	dst->text[idx][j] = '\0';
	return 0;
}

static int parse_text(struct subtitle *dst, size_t idx, const char *text)
{
	dst->text[idx] = strdup(text);
	if (dst->text[idx] == NULL) {
		stos_write_error("out of memory");
		return -1;
	}
	return 0;
}

static int parse_bitm(struct subtitle *dst, size_t idx)
{
	dst->text[idx] = NULL;
	return 0;
}

static int parse_sub(struct subtitle *dst, const AVSubtitle *sub,
		     const AVPacket *pkt)
{
	dst->text = NULL;
	dst->num_text = 0;
	dst->styled = 0;
	//https://ffmpeg.org/doxygen/trunk/group__lavf__decoding.html
	if (pkt->dts == AV_NOPTS_VALUE) {
		dst->start_time = sub->start_display_time;
		dst->end_time = sub->end_display_time;
	} else {
		dst->start_time = pkt->dts;
		dst->end_time = dst->start_time + pkt->duration;
	}
	
	if (sub->num_rects == 0)
		return 0;
	dst->text = malloc(sizeof(*dst->text) * sub->num_rects);
	if (dst->text == NULL) {
		stos_write_error("out of memory");
		goto error;
	}
	
	for (size_t idx = 0; idx < sub->num_rects; ++idx) {
		switch (sub->rects[idx]->type) {
		case SUBTITLE_BITMAP:
			parse_bitm(dst, idx);
			break;
		case SUBTITLE_TEXT:
			parse_text(dst, idx, sub->rects[idx]->text);
			break;
		case SUBTITLE_ASS:
			parse_ass(dst, idx, sub->rects[idx]->ass);
			break;
		default:
			stos_write_error("unsupported subtitle type");
			goto error;
		}
		dst->num_text += 1;
	}
	return 0;
error:
	del_sub(dst);
	dst->text = NULL;
	dst->num_text = 0;
	return -1;
}

static int read_sub_pkt(AVFormatContext *fctx, AVPacket *pkt)
{
	int ret = av_read_frame(fctx, pkt);
	if (ret == 0)
		return 1;
	return 0;
}

static struct subtitle *decode_subs(const struct file_info *info,
				    AVCodecContext *cctx, size_t *n)
{
	struct subtitle *subs = NULL;
	size_t count = 0;
	size_t size = 0;
	int ret;
	int got;
	
	AVPacket *pkt = av_packet_alloc();
	if (pkt == NULL) {
		stos_write_error("failed to allocate packet");
		goto error;
	}
	
	AVSubtitle sub;
	while ((ret = read_sub_pkt(info->fctx, pkt)) > 0) {
		if (avcodec_decode_subtitle2(cctx, &sub, &got, pkt) < 0) {
			stos_write_error("failed to decode subtitle");
			goto error;
		}
		if (count == size) {
			subs = realloc(subs,
				       sizeof(*subs) * ((size + 1) * 2));
			if (subs == NULL) {
				stos_write_error("out of memory");
				goto error;
			}
			size = (size + 1) * 2;
		}
		if (parse_sub(subs + count, &sub, pkt) < 0)
			goto error;
		if (got)
			avsubtitle_free(&sub);
		av_packet_unref(pkt);
		++count;
	}
	if (ret >= 0)
		goto cleanup;
error:
	//todo properly free subs as it is currently leaking when an
	//error occurs
	free(subs);
	subs = NULL;
	count = 0;
cleanup:
	if (pkt != NULL)
		av_packet_free(&pkt);
	if (n != NULL)
		*n = count;
	return subs;
}


//https://ffmpeg.org/doxygen/trunk/transcoding_8c-example.html#a24
struct subtitle* get_subs(const struct file_info *info, int stream_idx, size_t *n)
{
	const AVCodec *codec = NULL;
	AVStream *stream = NULL;
	struct subtitle *subs = NULL;

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
	
	AVCodecContext *cctx = avcodec_alloc_context3(codec);
	if (cctx == NULL) {
		stos_write_error("failed to allocate codec context");
		goto cleanup;
	}

	AVDictionary *opts = NULL;
	if (av_dict_set(&opts, "", "", 0) < 0) {
		stos_write_error("failed to set dictionary");
		goto cleanup;
	}
	if (avcodec_open2(cctx, codec, &opts) < 0) {
		stos_write_error("could not open2 codec");
		goto cleanup;
	}

	subs = decode_subs(info, cctx, n);
cleanup:
	if (cctx != NULL)
		avcodec_free_context(&cctx);
	if (opts != NULL)
		av_dict_free(&opts);
	return subs;
}

