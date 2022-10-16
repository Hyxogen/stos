#include <sub_new.c>
#include <assert.h>
#include <string.h>

int main(void)
{
	(void)stos_ass_extract;
	{
		const char *str = "asdf";
		const char *oth = stos_nstrchr(str, ',', (size_t)8);
		assert(oth == NULL);
	}
	{
		const char *str = "asdf";
		const char *cor = strchr(str, 'd');
		const char *tst = stos_nstrchr(str, 'd', 1);
		assert(cor == tst);
	}
	{
		const char *str = "asdf";
		const char *oth = stos_nstrchr(str, 's', 2);
		assert(oth == NULL);
	}
	{
		const char *str = "asdfasdf";
		const char *oth = stos_nstrchr(str, 'f', 2);
		assert(oth != NULL);
		assert(*oth == 'f');
	}
	{
		const char *str = "asdfasdf";
		const char *oth = stos_nstrchr(str, 's', 2);
		assert(strcmp(oth, "sdf") == 0);
	}
	{
		const char *str = "asdfasdf";
		const char *oth = stos_nstrchr(str, L'ｆ', 1);
		assert(oth == NULL);
	}
	{
		const char *str = "";
		const char *oth = stos_nstrchr(str, '\0', 1);
		assert(oth == str);
	}
	{
		const char *str = "asdfasdf";
		const char *oth = stos_nstrchr(str, 'f', 0);
		assert(oth == str);
	}
	{
		const char *str = "asdfasdf";
		const char *cor = strchr(str, '\0');
		const char *tst = stos_nstrchr(str, '\0', 1);
		assert(cor == tst);
	}
	return 0;
}
