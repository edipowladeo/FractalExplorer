#include "DebugObjects.h"
#include "PropriedadesJanela.h"


CDebugObjects::CDebugObjects(CpropriedadesJanela & Propriedades)
{

RtgDesenhar.setOutlineThickness(2);
RtgAlocar.setOutlineThickness(2);
RtgDesalocar.setOutlineThickness(2);

//cout << "DEBUG CTOR WITH PARAMETERs" << RtgAlocar.getSize().x; getchar();

RtgDesenhar.setFillColor(sf::Color(0,0,0,0));
RtgAlocar.setFillColor(sf::Color(0,0,0,0));
RtgDesalocar.setFillColor(sf::Color(0,0,0,0));

RtgDesenhar.setOutlineColor(sf::Color(150, 50, 250,255));        //SETUP INICIAL DO RETANGULO DO DESENVOLVEDOR
RtgAlocar.setOutlineColor(sf::Color(50, 150, 250,255));        //SETUP INICIAL DO RETANGULO DO DESENVOLVEDOR
RtgDesalocar.setOutlineColor(sf::Color(100, 250, 50,255));        //SETUP INICIAL DO RETANGULO DO DESENVOLVEDOR

Circulo_pivoTela.setRadius(10);
Circulo_pivoTela.setFillColor(sf::Color(255,0,0,128));
Circulo_pivoTela.setOrigin(10,10);

RtgPivoTela.setSize(sf::Vector2f(20,20));
RtgPivoTela.setFillColor(sf::Color(0,0,255,128));
RtgPivoTela.setOrigin(10,10);
RtgPivoTela.setOutlineThickness(-2);
RtgPivoTela.setOutlineColor(sf::Color(0, 255, 0));

Atualizar(Propriedades);
}


void CDebugObjects::Atualizar(const CpropriedadesJanela & Propriedades)
{
	RtgDesenhar.setSize(Propriedades.JanelaDesenho);
	RtgAlocar.setSize(Propriedades.JanelaAlocar);
	RtgDesalocar.setSize(Propriedades.JanelaDesalocar);

	RtgDesenhar.setOrigin(Propriedades.JanelaDesenho * 0.5);
	RtgAlocar.setOrigin(Propriedades.JanelaAlocar * 0.5);
	RtgDesalocar.setOrigin(Propriedades.JanelaDesalocar * 0.5);

	RtgDesenhar.setPosition(Propriedades.DimensoesJanela * 0.5);
	RtgAlocar.setPosition(Propriedades.DimensoesJanela * 0.5);
	RtgDesalocar.setPosition(Propriedades.DimensoesJanela * 0.5);


	if(fonteTexto.loadFromFile("NotoMono-Regular.ttf"))
	{
		textoDebug.setFont(fonteTexto);
	}
	else
	{
		cout << endl << "Erro abrindo fonte";
		getchar();
	}
	textoDebug.setCharacterSize(14);
	textoDebug.setFillColor(sf::Color::Black);
	textoDebug.setOutlineColor(sf::Color::White);
	textoDebug.setOutlineThickness(1.2);
	textoDebug.setPosition(sf::Vector2f(20, 10));
	textoDebug.setStyle(sf::Text::Bold);
}



CDebugObjects::CDebugObjects(){
  //  cout << "DEBUG CTOR WITHOUT PARAMETERs"; getchar();
}
