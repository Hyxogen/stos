#include <sub_new.c>

#include <string.h>
#include <assert.h>

int main(void) {
	{
		char *text;
		size_t len;
		int styled;
		assert(stos_ass_extract(&text, &len, &styled, "348,0,Default,,0,0,0,,息を合わせて…") == STOS_SUCCESS);
		assert(strcmp(text, "息を合わせて…") == 0);
		assert(styled == 0);
		free(text);
	}
	return 0;
}
