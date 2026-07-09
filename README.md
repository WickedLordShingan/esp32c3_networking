Tried fiddling around with esp_radio and smoltcp. Got it to connect to my home network with a custom mini async executor
— esp_radio only has non-blocking async_connect in its api — since I was reluctanct to use embassy. But the amount of errors that kept 
piling up shattered my resolve to continue doing this so I used embassy. That is another repo coz I wanna honor my efforts using this one
