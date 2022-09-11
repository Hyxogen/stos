TARGET		:= stos

CC		:= cc

SOURCE_FILES	:= main.c
OBJECT_FILES	:= $(SOURCE_FILES:.c=.o)

CFLAGS		:= -Wall -Wextra -std=c99 -pedantic -fsanitize=address
LFLAGS		:= -Wall -Wextra -std=c99 -pedantic -lavcodec -lavutil \
		   -lavformat -fsanitize=address

all: $(TARGET)

$(TARGET): $(OBJECT_FILES)
	$(CC) -o $(TARGET) $(OBJECT_FILES) $(LFLAGS)

%.o: %.c Makefile
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OBJECT_FILES)
	rm -f $(TARGET)
