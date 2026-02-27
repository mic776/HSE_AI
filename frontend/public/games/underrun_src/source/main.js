
audio_init(function(){
	terminal_cancel();
	terminal_hide();
	renderer_init();
	load_image('q2', function() {
		renderer_bind_image(this);
		next_level(game_tick);
	});
});

