#include "PropriedadesJanela.h"

CpropriedadesJanela::CpropriedadesJanela()
{
	PosicaoInicial.delta = pow(2, -MagInicial);
	PosicaoInicial.x = -0.6;
	PosicaoInicial.y = 0;

	MagAlocarMax = 1;
	MagAlocarMin = -1;
	MagDesalocarMax = 3;
	MagDesalocarMin = -9;

	RazaoTamJanelaAlocar = 1;
	RazaoTamJanelaDesenho = 1;
	RazaoTamJanelaDesalocar = 1.2;

	RazaoJanelasDebug = 0.5;

	TaxaZoom = 3;
	TaxaPan = 128;

	UsarShader = true;

	DimensoesJanela = sf::Vector2f(1000, 700);
	DimensoesTextura = CCoordenadas2DI(64, 64);
	Recobrimento = CCoordenadas2DI(4, 4);
	DimensoesMatrizCelulas = CCoordenadas2DI(200, 200);
	DebugCelulas = false;
	DebugCamadas = false;
	telaCheia = false;
	AtualizarLimites();
	//cout << "CALL DEFAULT "; getchar();
}

void CpropriedadesJanela::AtualizarLimites()
{
	float escala = 1;
	if(DebugCelulas) escala = RazaoJanelasDebug;

	JanelaDesenho = DimensoesJanela * (RazaoTamJanelaDesenho * escala);
	JanelaAlocar = DimensoesJanela * (RazaoTamJanelaAlocar * escala);
	JanelaDesalocar = DimensoesJanela * (RazaoTamJanelaDesalocar * escala);
}
