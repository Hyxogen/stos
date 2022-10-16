#include <sub_new.c>

#include <stddef.h>
#include <string.h>
#include <assert.h>

int main(void)
{
	{
		char *text;
		size_t len;
		int styled;
		assert(stos_ass_extract(&text, &len, &styled,
					"348,0,Default,,0,0,0,,息を合わせて…") ==
		       STOS_SUCCESS);
		assert(strcmp(text, "息を合わせて…") == 0);
		assert(styled == 0);
		assert(len == strlen(text));
		free(text);
	}
	{
		assert(stos_ass_extract(NULL, NULL, NULL,
					"348,0,Default,,0,0,0,,息を合わせて…") ==
		       STOS_SUCCESS);
	}
	{
		size_t len;
		assert(stos_ass_extract(NULL, &len, NULL,
					"348,0,Default,,0,0,0,,息を合わせて…") ==
		       STOS_SUCCESS);
		assert(len == strlen("息を合わせて…"));
	}
	{
		char *text;
		size_t len;
		assert(stos_ass_extract(&text, &len, NULL,
					"348,0,Default,,0,0,0,,") ==
		       STOS_SUCCESS);
		assert(len == 0);
		assert(*text == '\0');
		free(text);
	}
	{
		int styled;
		assert(stos_ass_extract(
			       NULL, NULL, &styled,
			       "348,0,Default,,0,0,0,,{\\i1}Hello There{\\i0}") ==
		       STOS_SUCCESS);
		assert(styled == 1);
	}
	return 0;
}
