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
#include <assert.h>
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>

#define stos_assert(x)     \
	do {               \
		assert(x); \
	} while (0)

const char *stos_get_error(enum stos_error error)
{
	switch (error) {
	case STOS_SUCCESS:
		return "no error";
	case STOS_OUT_OF_MEMORY:
		return "out of memory";
	case STOS_COULD_NOT_OPEN:
		return "could not open a stream or a file";
	case STOS_NO_INFO:
		return "no information could be retrieved about the file";
	case STOS_NO_STREAM:
		return "the file does not have a stream";
	case STOS_INVALID_FORMAT:
		return "the contents of the file is not properly formatted";
	case STOS_COULD_NOT_DECODE:
		return "unable to decode the subtitle";
	case STOS_UNSUPPORTED:
		return "stos currently does not support this";
	case STOS_END_OF_STREAM:
		return "the end of the stream was reached";
	case STOS_UNKNOWN:
		return "an unknown error occurred, please make an issue of this on the repository page";
	default:
		return "incorrect error code";
	}
}

enum stos_error get_file_info(struct file_info *info, const char *url)
{
	enum stos_error error;
	
	info->fctx = NULL;
	if (avformat_open_input(&info->fctx, url, NULL, NULL) < 0) {
		error = STOS_COULD_NOT_OPEN;
		goto error;
	}
	if (avformat_find_stream_info(info->fctx, NULL) < 0) {
		error = STOS_NO_INFO;
		goto error;
	}
	return STOS_SUCCESS;
error:
	if (info->fctx != NULL)
		avformat_close_input(&info->fctx);
	return error;
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

/* Perhaps make this take a pointer-pointer to subs to indicate that
 * this function takes ownership of subs?
 */
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

static enum stos_error parse_ass(struct subtitle *dst, size_t idx,
				 const char *event)
{
	size_t i = 0;
	size_t j = 0;
	size_t brackets = 0;
	size_t len = 0;

	event = nstrchr(event, ',', 8);
	if (event == NULL || *event == '\0')
		return STOS_INVALID_FORMAT;

	len = strlen(event);
	dst->text[idx] = malloc(len + 1);
	if (dst->text[idx] == NULL)
		return STOS_OUT_OF_MEMORY;
	while (i < len) {
		if (event[i] == '{') {
			dst->styled = 1;
			brackets += 1;
		} else if (event[i] == '}' && brackets != 0) {
			brackets -= 1;
		} else if (event[i] == '\\' && i + 1 < len &&
			   tolower(event[i]) == 'n') {
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

static enum stos_error parse_text(struct subtitle *dst, size_t idx,
				  const char *text)
{
	dst->text[idx] = strdup(text);
	if (dst->text[idx] == NULL)
		return STOS_OUT_OF_MEMORY;
	return STOS_SUCCESS;
}

static enum stos_error parse_bitm(struct subtitle *dst, size_t idx)
{
	dst->text[idx] = NULL;
	return STOS_SUCCESS;
}

static enum stos_error parse_sub(struct subtitle *dst, const AVSubtitle *sub)
{
	enum stos_error error = STOS_SUCCESS;
	dst->text = NULL;
	dst->num_text = 0;
	dst->styled = 0;

	dst->start_time = sub->start_display_time;
	dst->end_time = sub->end_display_time;
	
	if (sub->num_rects == 0)
		return 0;
	dst->text = malloc(sizeof(*dst->text) * sub->num_rects);
	if (dst->text == NULL) {
		error = STOS_OUT_OF_MEMORY;
		goto cleanup;
	}

	for (size_t idx = 0; idx < sub->num_rects; ++idx) {
		dst->text[idx] = NULL;
		
		if (error != STOS_SUCCESS)
			continue;
		
		switch (sub->rects[idx]->type) {
		case SUBTITLE_BITMAP:
			error = parse_bitm(dst, idx);
			break;
		case SUBTITLE_TEXT:
			error = parse_text(dst, idx, sub->rects[idx]->text);
			break;
		case SUBTITLE_ASS:
			error = parse_ass(dst, idx, sub->rects[idx]->ass);
			if (error == STOS_INVALID_FORMAT)
				error= STOS_SUCCESS;
			break;
		default:
			error = STOS_INVALID_FORMAT;
		}
		dst->num_text += 1;
	}
cleanup:
	if (error != STOS_SUCCESS) {
		del_sub(dst);
		dst->text = NULL;
		dst->num_text =0;
	}
	return error;
}

static enum stos_error read_sub_pkt(AVFormatContext *fctx, AVPacket *pkt)
{
	int ret = av_read_frame(fctx, pkt);
	if (ret == 0)
		return STOS_SUCCESS;
	/* TODO make this an error and try to check for end of stream
	   in the decoding stage (with avcodec_decode_subtitle2)*/
	return STOS_END_OF_STREAM;
}

//https://ffmpeg.org/doxygen/trunk/group__lavf__decoding.html
static void sub_fix_timings(AVSubtitle *sub, const AVPacket *pkt)
{
	if (pkt->dts != AV_NOPTS_VALUE) {
		sub->start_display_time = pkt->dts;
		sub->end_display_time = pkt->pts;
	}
}

static enum stos_error read_sub_and_decode(AVFormatContext *fctx,
					   AVCodecContext *cctx, AVPacket *pkt,
					   AVSubtitle *sub)
{
	enum stos_error error;

	error = read_sub_pkt(fctx, pkt);
	if (error != STOS_SUCCESS)
		return error;

	int got, rc;
	rc = avcodec_decode_subtitle2(cctx, sub, &got, pkt);
	if (rc == AVERROR_INVALIDDATA)
		error = STOS_INVALID_FORMAT;
	else if (rc < 0)
		error = STOS_UNKNOWN;
	else if (got == 0)
		error = STOS_UNSUPPORTED;

	sub_fix_timings(sub, pkt);

	av_packet_unref(pkt);
	return error;
}

static enum stos_error decode_subs(struct subtitle **out,
				   const struct file_info *info,
				   AVCodecContext *cctx, size_t *n)

{
	size_t count = 0;
	size_t size = 0;
	enum stos_error error = STOS_SUCCESS;
	struct subtitle *subs = NULL;
	AVSubtitle sub;
	
	AVPacket *pkt = av_packet_alloc();
	if (pkt == NULL)
		return STOS_OUT_OF_MEMORY;

	/* TODO check if I can deallocate the packet after decoding */
	while (error == STOS_SUCCESS) {
		error = read_sub_and_decode(info->fctx, cctx, pkt, &sub);
		if (error != STOS_SUCCESS) {
			if (error == STOS_INVALID_FORMAT)
				error = STOS_SUCCESS;
			continue;
		}
		

		if (count == size) {
			size_t new_size = (size + 1) * 2;
			subs = realloc(subs, sizeof(*subs) * new_size);
			if (subs == NULL) {
				error = STOS_OUT_OF_MEMORY;
				goto cleanup_and_loop;
			}
			size = new_size;
		}

		error = parse_sub(subs + count, &sub);
		if (error != STOS_SUCCESS)
			goto cleanup_and_loop;

		count += 1;
cleanup_and_loop:
		avsubtitle_free(&sub);
	}
	if (error == STOS_SUCCESS || error == STOS_END_OF_STREAM) {
		error = STOS_SUCCESS;
		goto cleanup_and_return;
	}
	del_subs(subs, count);
	subs = NULL;
	count = 0;
cleanup_and_return:
	if (pkt != NULL)
		av_packet_free(&pkt);
	if (out != NULL)
		*out = subs;
	else
		free(subs);
	if (n != NULL)
		*n = count;
	return error;
}

//https://ffmpeg.org/doxygen/trunk/transcoding_8c-example.html#a24
enum stos_error get_subs(struct subtitle **out, const struct file_info *info,
			  int stream_idx, size_t *n)
{
	enum stos_error error = STOS_SUCCESS;
	const AVCodec *codec = NULL;
	AVStream *stream = NULL;

	if (stream_idx < 0) {
		stream = find_first_sub_stream(info);
	} else if ((unsigned int) stream_idx < info->fctx->nb_streams) {
		stream = info->fctx->streams[stream_idx];
	}

	if (stream == NULL) {
		error = STOS_NO_STREAM;
		goto cleanup;
	}

	codec = avcodec_find_decoder(stream->codecpar->codec_id);
	if (codec == NULL) {
		error = STOS_UNSUPPORTED;
		goto cleanup;
	}
	
	AVCodecContext *cctx = avcodec_alloc_context3(codec);
	if (cctx == NULL) {
		error = STOS_OUT_OF_MEMORY;
		goto cleanup;
	}

	AVDictionary *opts = NULL;
	if (av_dict_set(&opts, "", "", 0) < 0) {
		error = STOS_UNKNOWN;
		goto cleanup;
	}
	if (avcodec_open2(cctx, codec, &opts) < 0) {
		error = STOS_UNKNOWN;
		goto cleanup;
	}

	error = decode_subs(out, info, cctx, n);
cleanup:
	if (cctx != NULL)
		avcodec_free_context(&cctx);
	if (opts != NULL)
		av_dict_free(&opts);
	return error;
}

