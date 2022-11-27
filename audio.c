#include <stos.h>
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavfilter/buffersink.h>
#include <libavfilter/buffersrc.h>
#include <libavfilter/avfilter.h>
#include <libavutil/channel_layout.h>
#include <libavutil/opt.h>

static enum stos_error stos_decode_packet(struct stream *ist)
{
	int err = avcodec_send_packet(ist->codec_ctx, ist->pkt);
	if (err < 0)
		return STOS_EDECODE;
	return STOS_OK;
}

static enum stos_error stos_filter_packets(struct stream *ist)
{
	int err;

	while ((err = avcodec_receive_frame(ist->codec_ctx, ist->frame)) >= 0) {
		if (av_buffersrc_add_frame(ist->filter_ctx, ist->frame) <
		    0)
			return STOS_EFILTER;
	}
	if (err == AVERROR_EOF)
		return STOS_EOF;
	else if (err < 0 && err != AVERROR(EAGAIN))
		return STOS_EDECODE;
	return STOS_OK;
}

static enum stos_error stos_encode_frames(struct stream *ost)
{
	int err;

	while ((err = av_buffersink_get_frame(ost->filter_ctx, ost->frame)) >= 0) {
		if (avcodec_send_frame(ost->codec_ctx, ost->frame) < 0)
			return STOS_EENCODE;
	}
	if (err == AVERROR_EOF)
		return STOS_EOF;
	else if (err < 0 && err != AVERROR(EAGAIN))
		return STOS_EDECODE;
	return STOS_OK;
}

static enum stos_error stos_write_frames(struct file *ofile, struct stream *ost)
{
	int err;

	while ((err = avcodec_receive_packet(ost->codec_ctx, ost->pkt)) >= 0) {
		av_packet_rescale_ts(ost->pkt, ost->codec_ctx->time_base,
				     ost->stream->time_base);
		if (av_interleaved_write_frame(ofile->fmt, ost->pkt) < 0)
			return STOS_EIO;
	}
	if (err == AVERROR_EOF)
		return STOS_EOF;
	else if (err < 0 && err != AVERROR(EAGAIN))
		return STOS_EDECODE;
	return STOS_OK;
}

static enum stos_error stos_transcode(struct file *ifile,
				      struct stream *istream,
				      struct file *ofile,
				      struct stream *ostream)
{
	AVPacket *pkt = av_packet_alloc();
	AVFrame *frame = av_frame_alloc();
	enum stos_error err = STOS_OK;

	if (pkt == NULL || frame == NULL) {
		err = STOS_ENOMEM;
		goto end;
	}

	istream->pkt = pkt;
	istream->frame = frame;

	//This is probably not ok, unref will probably go wrong
	ostream->pkt = pkt;
	ostream->frame = frame;
	while (err == STOS_OK) {
		int rc = av_read_frame(ifile->fmt, pkt);
		if (rc == AVERROR_EOF) {
			err = STOS_EOF;
			goto loop;
		} else if (rc < 0) {
			err = STOS_EREAD_FRAME;
			break;
		}

		if (pkt->stream_index == istream->stream->index) {
			err = stos_decode_packet(istream);
			if (err == STOS_OK || err == STOS_EOF)
				err = stos_filter_packets(istream);
			if (err == STOS_OK || err == STOS_EOF)
				err = stos_encode_frames(ostream);
			if (err == STOS_OK || err == STOS_EOF)
				err = stos_write_frames(ofile, ostream);
		}
loop:
		av_packet_unref(pkt);
	}
end:
	if (pkt != NULL)
		av_packet_free(&pkt);
	if (frame != NULL)
		av_frame_free(&frame);
	return err;
}

enum stos_error stos_open_ifile(struct file *file, const char *url)
{
	file->fmt = NULL;
	if (avformat_open_input(&file->fmt, url, NULL, NULL) < 0)
		return STOS_EBADF;
	if (avformat_find_stream_info(file->fmt, NULL) < 0) {
		avformat_close_input(&file->fmt);
		return STOS_EIO;
	}
	return STOS_OK;
}

enum stos_error stos_open_ofile(struct file *file, const char *path)
{
	file->fmt = NULL;
	avformat_alloc_output_context2(&file->fmt, NULL, NULL, path);
	if (file->fmt == NULL)
		return STOS_ENOMEM;
	if (avio_open(&file->fmt->pb, path, AVIO_FLAG_WRITE) >= 0)
		return STOS_OK;
	avformat_free_context(file->fmt);
	return STOS_EIO;
}

enum stos_error stos_open_istream(struct file *ifile, struct stream *dst,
				  int stream_index)
{
	if (stream_index < 0 || (unsigned int)stream_index > ifile->fmt->nb_streams)
		return STOS_ENOSUB;

	AVStream *stream = ifile->fmt->streams[(unsigned int)stream_index];
	const AVCodec *codec = avcodec_find_decoder(stream->codecpar->codec_id);
	if (codec == NULL)
		return STOS_UNSUP;

	AVCodecContext *dec_ctx = avcodec_alloc_context3(codec);
	if (dec_ctx == NULL)
		return STOS_ENOMEM;

	enum stos_error status = STOS_EUNKNOWN;
	if (avcodec_parameters_to_context(dec_ctx, stream->codecpar) < 0)
		goto error;

	if (avcodec_open2(dec_ctx, codec, NULL))
		goto error;

	struct stream istream = { .stream = stream,
				  .codec = codec,
				  .codec_ctx = dec_ctx };

	*dst = istream;
	return STOS_OK;
error:
	if (dec_ctx != NULL)
		avcodec_free_context(&dec_ctx);
	return status;
}

enum stos_error stos_open_ostream(struct file *ofile, struct stream *ostream,
				  const struct stream *istream)
{
	ostream->stream = avformat_new_stream(ofile->fmt, NULL);
	if (ostream->stream == NULL)
		return STOS_ENOMEM;

	enum stos_error err = STOS_EUNKNOWN;

	ostream->codec = avcodec_find_encoder(istream->codec_ctx->codec_id);
	if (ostream->codec == NULL) {
		err = STOS_UNSUP;
		goto error;
	}

	ostream->codec_ctx = avcodec_alloc_context3(ostream->codec);
	if (ostream->codec_ctx == NULL) {
		err = STOS_ENOMEM;
		goto error;
	}

	ostream->codec_ctx->sample_rate = istream->codec_ctx->sample_rate;
	if (av_channel_layout_copy(&ostream->codec_ctx->ch_layout,
				   &istream->codec_ctx->ch_layout) < 0)
		goto error;

	//TODO get rid of the magic number 0 here
	ostream->codec_ctx->sample_fmt = ostream->codec->sample_fmts[0];
	ostream->codec_ctx->time_base =
		(AVRational){ 1, ostream->codec_ctx->sample_rate };

	if (avcodec_open2(ostream->codec_ctx, ostream->codec, NULL) < 0)
		goto error;

	if (avcodec_parameters_from_context(ostream->stream->codecpar,
					    ostream->codec_ctx) < 0)
		goto error;
	return STOS_OK;
error:
	if (ostream->codec_ctx != NULL)
		avcodec_free_context(&ostream->codec_ctx);
	return err;
}

enum stos_error stos_setup_filters(struct stream *istream,
				   struct stream *ostream)
{
	istream->filter_graph = avfilter_graph_alloc();
	if (!istream->filter_graph)
		return STOS_ENOMEM;

	enum stos_error err = STOS_EUNKNOWN;
	AVFilterInOut *outputs = NULL;
	AVFilterInOut *inputs = NULL;

	const AVFilter *abuffer = avfilter_get_by_name("abuffer");
	const AVFilter *abuffersink = avfilter_get_by_name("abuffersink");
	if (!abuffer || !abuffersink) {
		err = STOS_EINVAL;
		goto error;
	}

	if (istream->codec_ctx->ch_layout.order == AV_CHANNEL_ORDER_UNSPEC) {
		av_channel_layout_default(
			&istream->codec_ctx->ch_layout,
			istream->codec_ctx->ch_layout.nb_channels);
	}
	/* TODO remove these weird temporary buffers */
	char buf[64];
	char args[512];
	av_channel_layout_describe(&istream->codec_ctx->ch_layout, buf,
				   sizeof(buf));
	snprintf(
		args, sizeof(args),
		"time_base=%d/%d:sample_rate=%d:sample_fmt=%s:channel_layout=%s",
		istream->codec_ctx->time_base.num,
		istream->codec_ctx->time_base.den,
		istream->codec_ctx->sample_rate,
		av_get_sample_fmt_name(istream->codec_ctx->sample_fmt), buf);
	if (avfilter_graph_create_filter(&istream->filter_ctx, abuffer, "in",
					 args, NULL,
					 istream->filter_graph) < 0) {
		err = STOS_EFILTER;
		goto error;
	}

	if (avfilter_graph_create_filter(&ostream->filter_ctx, abuffersink,
					 "out", NULL, NULL,
					 istream->filter_graph) < 0) {
		err = STOS_EFILTER;
		goto error;
	}

	if (av_opt_set_bin(ostream->filter_ctx, "sample_fmts",
			   (unsigned char *)&ostream->codec_ctx->sample_fmt,
			   sizeof(ostream->codec_ctx->sample_fmt),
			   AV_OPT_SEARCH_CHILDREN) < 0) {
		err = STOS_EOPT;
		goto error;
	}
	av_channel_layout_describe(&ostream->codec_ctx->ch_layout, buf,
				   sizeof(buf));

	if (av_opt_set(ostream->filter_ctx, "ch_layouts", buf,
		       AV_OPT_SEARCH_CHILDREN) < 0) {
		err = STOS_EOPT;
		goto error;
	}

	if (av_opt_set_bin(ostream->filter_ctx, "sample_rates",
			   (unsigned char *)&ostream->codec_ctx->sample_rate,
			   sizeof(ostream->codec_ctx->sample_rate),
			   AV_OPT_SEARCH_CHILDREN) < 0) {
		err = STOS_EOPT;
		goto error;
	}

	outputs = avfilter_inout_alloc();
	inputs = avfilter_inout_alloc();
	if (!outputs || !inputs) {
		err = STOS_ENOMEM;
		goto error;
	}

	outputs->name = av_strdup("in");
	outputs->filter_ctx = istream->filter_ctx;
	outputs->pad_idx = 0;
	outputs->next = NULL;

	inputs->name = av_strdup("out");
	inputs->filter_ctx = ostream->filter_ctx;
	inputs->pad_idx = 0;
	inputs->next = NULL;

	if (avfilter_graph_parse_ptr(istream->filter_graph, "anull", &inputs,
				     &outputs, NULL) < 0)
		goto error;
	if (avfilter_graph_config(istream->filter_graph, NULL) < 0)
		goto error;
	istream->inout = inputs;
	ostream->inout = outputs;
	return STOS_OK;
error:
	if (outputs)
		avfilter_inout_free(&outputs);
	if (inputs)
		avfilter_inout_free(&inputs);
	return err;
}

static void stos_close_stream(struct stream *stream)
{
	if (stream->codec_ctx)
		avcodec_free_context(&stream->codec_ctx);
	stream->codec_ctx = NULL;
	if (stream->inout)
		avfilter_inout_free(&stream->inout);
	stream->inout = NULL;
	stream->stream = NULL;
	stream->filter_ctx = NULL;
	stream->codec = NULL;
}

void stos_stream_init(struct stream *stream)
{
	stream->stream = NULL;
	stream->codec_ctx = NULL;
	stream->codec = NULL;
	stream->filter_graph = NULL;
	stream->filter_ctx = NULL;
	stream->inout = NULL;
	stream->pkt = NULL;
	stream->frame = NULL;
}

void stos_init_file(struct file *file)
{
	file->fmt = NULL;
	file->close_separately = 0;
}

void stos_close_file(struct file *file)
{
	if (file->fmt) {
		if (file->close_separately)
			avio_closep(&file->fmt->pb);
		avformat_close_input(&file->fmt);
	}
}

int stos_find_stream(const struct file *file,
		     int (*predicate)(const AVStream *))
{
	const int max = file->fmt->nb_streams >= INT_MAX ?
				INT_MAX :
				(int)file->fmt->nb_streams;
	for (int idx = 0; idx < max; ++idx) {
		if (predicate(file->fmt->streams[idx]))
			return idx;
	}
	return -1;
}

static int stos_is_audio_stream(const AVStream *stream)
{
	return stream->codecpar->codec_type == AVMEDIA_TYPE_AUDIO;
}

enum stos_error stos_audio_do(struct file *ifile, struct file *ofile)
{
	struct stream istream;
	struct stream ostream;

	int stream_index = stos_find_stream(ifile, stos_is_audio_stream);
	if (stream_index < 0)
		return STOS_NOSTREAM;

	stos_stream_init(&istream);
	stos_stream_init(&ostream);

	enum stos_error status = stos_open_istream(ifile, &istream, stream_index);
	if (status != STOS_OK)
		return status;

	status = stos_open_ostream(ofile, &ostream, &istream);
	if (status != STOS_OK)
		goto cleanup;

	status = stos_setup_filters(&istream, &ostream);
	if (status != STOS_OK)
		goto cleanup;


	if (avformat_write_header(ofile->fmt, NULL) >= 0)
		status = stos_transcode(ifile, &istream, ofile, &ostream);
	else
		status = STOS_EIO;
cleanup:
	stos_close_stream(&istream);
	stos_close_stream(&ostream);
	return status;
}
