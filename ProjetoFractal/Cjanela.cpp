#include "Cjanela.h"

Cjanela::Cjanela(Cprograma* Programa)
{
	//CONFIGURAÇÕES INICIAIS

	//FIM CONFIGURACOES

	this->Programa = Programa;
	DebugObjects = CDebugObjects(Propriedades);

	pivoAtual = Propriedades.PosicaoInicial;
	pivoDesejado = pivoAtual;

	// Camera_desejada.delta = (double_min * pow(2,MAG_INTEIRA_INICIAL));
	// printf("\n pow(2,MAG_INTEIRA_INICIAL) %f", pow(2,MAG_INTEIRA_INICIAL));

	DeltaAtual = Propriedades.PosicaoInicial.delta;// 0.003;//0.00006103515;
	DeltaDesejado = DeltaAtual;

	// DeltaAtual = DeltaAtual;
	pivoTela.x = Propriedades.DimensoesJanela.x / 2;
	pivoTela.y = Propriedades.DimensoesJanela.y / 2;

	Paleta.precalcula_paleta(); //Crialookup table

	JanelaSFML = new sf::RenderWindow();
	JanelaSFML->create(sf::VideoMode(Propriedades.DimensoesJanela.x, Propriedades.DimensoesJanela.y, 32), "MandelCL", Propriedades.telaCheia ? sf::Style::Fullscreen : sf::Style::Default);

	Paleta.JanelaMae = this;

	if (ShaderDisponivel)
	{
		inicializar_shader();
	}

}

ConfigJanela::ConfigJanela(){

}

void  Cjanela::inicializar_shader()
{

	// TODOS UNIFORMS SÃO FLOAT, false SEI TRABALHOR COM INTEIRO
	// gl_FragColor =   vec4 (R,G,B,A);
	const std::string fragmentShader2 = \
		"uniform sampler2D currentTexture;"\
		"uniform float linhas_paleta;"\
		"uniform float largura;"
		"uniform float altura;"
		"uniform float offset;"
		"uniform float tam_paleta;"
		"uniform float sampling;"
		"uniform float transparencia;"
		"void main() {"\
		" vec4  pixel_color = vec4(0,0,0,0);"
		" float altura_total = altura + linhas_paleta;"
		" vec2  coord = gl_TexCoord[0].xy;"

		"  if (gl_TexCoord[0].y <  (altura/altura_total)) {"
		" vec4  cor_original = texture2D(currentTexture, coord);"
		" float mag =  cor_original.w*4294967296 cor_original.z*16777216 +cor_original.y*65536+ cor_original.x*256;"
		" mag /= sampling;"
		" mag += offset;"
		" mag = mag -  ((tam_paleta) *  floor(mag/(tam_paleta)));" // mag = mag%(tam_paleta)+1
		" float Y = floor(mag/largura);"                  //dy = (int)mag/largura
		" float X = mag - (largura*Y);"  //dx = mag%largura
		" float x_norm = (X + 0.5)/largura;"
		" float y_norm = (Y +altura + 0.5)/altura_total;"
		"vec2 coord_norm = vec2(x_norm,y_norm);"
		"  pixel_color += texture2D(currentTexture, coord_norm);}"
		"    gl_FragColor =   vec4 (pixel_color.xyz,0.1);"\
		"}";

	const std::string Shaderinterpolado = \
		"uniform sampler2D currentTexture;"\
		"uniform float linhas_paleta;"\
		"uniform float largura;"
		"uniform float altura;"
		"uniform float offset;"
		"uniform float tam_paleta;"
		"uniform float sampling;"
		"uniform float transparencia;"

		"void main() {"\
		" vec4  pixel_color1 = vec4(0,0,0,0);"
		" float altura_total = altura + linhas_paleta;"
		" vec2  coord = gl_TexCoord[0].xy;"

		" if (gl_TexCoord[0].y <  (altura/altura_total)) {"
		" vec4  cor_original = texture2D(currentTexture, coord);"
		" float mag =  cor_original.z*16777216 +cor_original.y*65536+ cor_original.x*256;"


		" mag /= sampling;"
		" mag += offset;"

		" mag = mag -  ((tam_paleta) *  floor(mag/(tam_paleta)));" // mag = mag%(tam_paleta)+1
		" float peso = mag - floor(mag);"
		" mag = floor(mag);"
		" float Y = floor(mag/largura);"                  //dy = (int)mag/largura
		" float X = mag - (largura*Y);"  //dx = mag%largura
		" float x_norm = (X + 0.5)/largura;"
		" float y_norm = (Y +altura + 0.5)/altura_total;"
		"vec2 coord_norm = vec2(x_norm,y_norm);"
		//  " float peso = X +0.5 - floor(X+.5);"
		"  pixel_color1 += texture2D(currentTexture, coord_norm)*(1-peso);"

		" mag +=1 ;"
		" mag = mag -  ((tam_paleta) *  floor(mag/(tam_paleta)));" // mag = mag%(tam_paleta)+1
		" Y = floor(mag/largura);"                  //dy = (int)mag/largura
		" X = mag - (largura*Y);"  //dx = mag%largura
		" x_norm = (X + 0.5)/largura;"
		" y_norm = (Y +altura + 0.5)/altura_total;"
		" coord_norm = vec2(x_norm,y_norm);"
		" pixel_color1 += texture2D(currentTexture, coord_norm)*(peso);"
		" pixel_color1 -= (0,0,0,transparencia);"
		"}"
		//   "if (mag > 1){ pixel_color1 -= vec4(1,1,1,0);}"
		"    gl_FragColor =   vec4 (pixel_color1.xyzw);"\
		"}";

	if (!sf::Shader::isAvailable())
	{
		std::cerr << "Shader are not available" << std::endl;
	}

	// Load shaders
	if (!shader.loadFromMemory(Shaderinterpolado, sf::Shader::Fragment))
	{
		std::cerr << "Error while shaders" << std::endl;
	}

	shader.setUniform("largura", (float)largura_textura);
	shader.setUniform("altura", (float)altura_textura);
	shader.setUniform("linhas_paleta", (float)LINHAS_ADICIONAIS_TEXTURA);
	shader.setUniform("sampling", (float)SAMPLING_ITERACOES);
	shader.setUniform("transparencia", (float)(Propriedades.DebugCelulas ? 0.4 : 0.0));

}

CCoordenadas2DTela Cjanela::CoordenadasPlanoDesejadoParaTela(CCoordenadas2DPlano Coordenada) //RETORNA A COORDENADA NA TELA DE UM SPRITE (i,j), LOCALIZADO NA SubANDA Sub SENDO EXIBIDO NA Camera Camera
{
	CCoordenadas2DTela Coord;
	Coord.x = pivoTela.x + (Coordenada.x - pivoDesejado.x) / DeltaDesejado;
	Coord.y = pivoTela.y + (Coordenada.y - pivoDesejado.y) / DeltaDesejado;
	return Coord;
}

CCoordenadas2DTela Cjanela::CoordenadasPlanoAtualParaTela(CCoordenadas2DPlano Coordenada) //RETORNA A COORDENADA NA TELA DE UM SPRITE (i,j), LOCALIZADO NA SubANDA Sub SENDO EXIBIDO NA Camera Camera
{
	CCoordenadas2DTela Coord;
	Coord.x = pivoTela.x + (Coordenada.x - pivoAtual.x) / DeltaAtual;
	Coord.y = pivoTela.y + (Coordenada.y - pivoAtual.y) / DeltaAtual;
	return Coord;
}

CCoordenadas2DTela Cjanela::CoordenadasPlanoAtualParaTelaDelta(CcoordenadasPlano Coordenada) //RETORNA A COORDENADA NA TELA DE UM SPRITE (i,j), LOCALIZADO NA SubANDA Sub SENDO EXIBIDO NA Camera Camera
{
	CCoordenadas2DTela Coord;
	Coord.x = pivoTela.x + (Coordenada.x - pivoAtual.x) / Coordenada.delta;
	Coord.y = pivoTela.y + (Coordenada.y - pivoAtual.y) / Coordenada.delta;
	return Coord;
}

CCoordenadas2DPlano Cjanela::CoordenadasTelaParaPlanoAtual(CCoordenadas2DTela CoordTela) {
	CCoordenadas2DPlano CoordPlano;
	CoordPlano.x = pivoAtual.x + (CoordTela.x - pivoTela.x) * DeltaAtual;
	CoordPlano.y = pivoAtual.y + (CoordTela.y - pivoTela.y) * DeltaAtual;
	return CoordPlano;
};

void Cjanela::alterarPivoTela(TCoordTela x, TCoordTela y) {
	CCoordenadas2DTela novoPivo(x, y);
	//IMPORTANTE NÃO ALTERAR PivoTela ANTES DA FUNCAO CoordenadasTelaParaPlanoAtual
	pivoAtual = CoordenadasTelaParaPlanoAtual(novoPivo);
	pivoDesejado = pivoAtual;
	pivoTela = novoPivo;
};

void Cjanela::verificarSeInsereCamadas() {
	if (!Camadas.empty()) {
		float logdeltadesejado = -log2(DeltaDesejado);
		float logfront = (Camadas.begin())->first;
		float logback = (--Camadas.end())->first;
		MagnificacaoMax = logdeltadesejado + Propriedades.MagAlocarMax;
		MagnificacaoMin = logdeltadesejado + Propriedades.MagAlocarMin;

		// cout << "total de " << Camadas.size() << " camadas" << endl;
		// cout <<"Verificando: logfronto = " <<logfront << " logback " << logback << endl;
		// cout <<  "     logdeltadesejado = " << logdeltadesejado << endl;

	  //   cout <<  "Mmin = " << MagnificacaoMin <<" Mmax = " << MagnificacaoMax << endl;

		if (MagnificacaoMax > logback) {
			cria_nova_Subcamada(logback + 1);
		}
		if ((MagnificacaoMin < logfront) && (logfront > 0)) {
			cria_nova_Subcamada(logfront - 1);
		}
	}
	else {
		cout << "VAI CRIRAR CAMADA PQ ESTÀ VAZIA ";
		cria_nova_Subcamada(Propriedades.MagInicial);

	}

	//    if ((logdelta)<logdeltadesejado){

	 //   }
}

void Cjanela::cria_nova_Subcamada(unsigned long int magnificacao)
{
	//    cout << "CRIOU SUBCAMADA" << endl;
		// Benchmark1.inicio(10);
	//    apagar_todas_tarefas(Janela);
	//    Programa_principal.apagar_todas_tarefas_livres_front();
	  //  if (inicializar == 0)

	  //Csubcamada * NovaSubcamada((this,magnificacao));
	if(Camadas.count(magnificacao) == 0)
	{
		CcoordenadasPlano Coordenadas;
		if(Camadas.size() == 0)
		{

		}
		else
		{
			Coordenadas = (*Camadas.begin()).second->Coordenadas;
		}
		Camadas.emplace(magnificacao, (new Csubcamada(this, magnificacao, Coordenadas)));
	}
	else { cout << endl << "TENTANDO EMPLACE CAMADA QUE JA EXISTE" << endl; }
	// Benchmark1.fim(10);
	// lista_camadas_tela(Janela,gui);
	// inicializar++; //faz com que só se crie camada uma vez
//Benchmark1.relatorio(10,true);
}

bool Cjanela::celula_esta_dentro(Ccelula* Celula, sf::Vector2f& JanelaVerificada)
{
	CCoordenadas2DTela Coord_tela = CoordenadasPlanoAtualParaTela(Celula->coordenadas_canto);

	float escalaCelula = Celula->coordenadas_canto.delta / DeltaAtual;
	float larguraTexturaNaTela = largura_textura * escalaCelula;
	float alturaTexturaNaTela = altura_textura * escalaCelula;

	Coord_tela.x *= 2;
	Coord_tela.y *= 2;

	Coord_tela.x += (larguraTexturaNaTela - Propriedades.DimensoesJanela.x);
	Coord_tela.y += (alturaTexturaNaTela - Propriedades.DimensoesJanela.y);

	if ((abs(Coord_tela.x) < (JanelaVerificada.x + larguraTexturaNaTela))
		&& (abs(Coord_tela.y) < (JanelaVerificada.y + alturaTexturaNaTela))) {
		return true;
	}

	else return false;
}

bool Cjanela::celula_estara_dentro(CcoordenadasPlano Coord, sf::Vector2f& JanelaVerificada)
{
	CCoordenadas2DTela Coord_tela = CoordenadasPlanoAtualParaTela(Coord);

	float EscalaCelula = Coord.delta / DeltaAtual;
	float larguraTexturaNaTela = largura_textura * EscalaCelula;
	float alturaTexturaNaTela = altura_textura * EscalaCelula;

	Coord_tela.x *= 2;
	Coord_tela.y *= 2;

	Coord_tela.x += (larguraTexturaNaTela - Propriedades.DimensoesJanela.x);
	Coord_tela.y += (alturaTexturaNaTela - Propriedades.DimensoesJanela.y);

	if ((abs(Coord_tela.x) < (JanelaVerificada.x + larguraTexturaNaTela))
		&& (abs(Coord_tela.y) < (JanelaVerificada.y + alturaTexturaNaTela))) {
		return true;
	}

	else return false;
}

void Cjanela::ToggleFullScreen()
{
	/*CCoordenadas2DI novoTamanho(JanelaSFML->getSize());
	Propriedades.JanelaAlocar.x = novoTamanho.x*0.5;
	Propriedades.JanelaAlocar.y = novoTamanho.y*0.5;
	sf::View novaView = JanelaSFML->getDefaultView();
	ssdebug << endl << "VIEW       getsize X " << novaView.getSize().x << " Y " << novaView.getSize().y;
	ssdebug << endl << "novaView   getsize X " << novaView.getCenter().x << " Y " << novaView.getCenter().y;
	ssdebug << endl << "novaView   getsize X " << novaView.getCenter().x << " Y " << novaView.getCenter().y;
	//ssdebug << endl << "novaView   getsize X " << novaView.getViewport().x << " Y " << novaView.getCenter().y;
	//sf::Vector2f =
	sf::FloatRect RECT = novaView.getViewport();
	ssdebug << endl << "novaView   getViewport X " << RECT.width << " Y " << RECT.height;

	
	novaView.setSize(sf::Vector2f(JanelaSFML->getSize()));
	ssdebug << endl << "VIEW  getsize after X " << novaView.getSize().x << " Y " << novaView.getSize().y;
	//novaView.*/
	//JanelaSFML->getSettings
	Propriedades.DimensoesJanela = sf::Vector2f(JanelaSFML->getSize());
	sf::FloatRect retanguloVisivel(sf::Vector2f(0.f, 0.f),Propriedades.DimensoesJanela);

	//sf::FloatRect 

	JanelaSFML->setView(sf::View(retanguloVisivel));
	Propriedades.AtualizarLimites();
	DebugObjects.Atualizar(Propriedades);
	/*
	ssdebug << endl << "wind       getsize X " << JanelaSFML->getSize().x << " Y " << JanelaSFML->getSize().y;
	ssdebug << endl << "wind  getposcenter X " << JanelaSFML->getPosition().x << " Y " << JanelaSFML->getPosition().y;
	*/
}

int Cjanela::NumeroDeTarefasPrepararCelula() {
	int sum = 0;
	for (auto ItrCamada = Camadas.begin(); ItrCamada != Camadas.end(); ItrCamada++) {
		sum += (*ItrCamada).second->TarefasAlocarCelulaPresentes.size();
	}
	return sum;
}

int Cjanela::NumeroDeTarefasFront() {
	int sum = 0;
	for (auto ItrCamada = Camadas.begin(); ItrCamada != Camadas.end(); ItrCamada++) {
		sum += (*ItrCamada).second->TarefasFrontPresentes.size();
	}
	return sum;
}

void Cjanela::atualiza_texturas()
{
	for (auto ItrSubcamada = Camadas.begin(); ItrSubcamada != Camadas.end(); ++ItrSubcamada)
	{
		(*ItrSubcamada).second->atualiza_textura();

	}
}

void Cjanela::plota_sprites()
{
	ToggleFullScreen();

	JanelaSFML->clear();
	shader.setUniform("transparencia", (float)(Propriedades.DebugCelulas ? 0.4 : 0.0));

	pivoAtual.x += (pivoDesejado.x - pivoAtual.x) / 15;
	pivoAtual.y += (pivoDesejado.y - pivoAtual.y) / 15;

	//quando DeltaAtual está quase alcançando delta desejado, surgem artefatos;
	//uma solução é interromper o processo de zoom um pouco antes, quando a razao entre eles está entre 1+-0.01
	float razao_escala = DeltaDesejado / DeltaAtual;
	// float desvio = razao_escala-1;
	// if (abs(desvio)>0.01)
	DeltaAtual *= pow(razao_escala, 0.1);

	if (Paleta.circular_cores)
	{
		//Paleta.offset_paleta += Paleta.velocidade_cores*RESOLUCAO_CORES;
		Paleta.offset_paleta = Paleta.offset_paleta + 1 % Paleta.tamanho_pal(Paleta.paleta_atual);
		if (!USAR_SHADER)
		{
			// Benchmark1.inicio(2);
			atualiza_texturas();
			// Benchmark1.fim(2);
			// Benchmark1.setscore(2,largura_textura*largura_textura);
		}
	}

	shader.setUniform("tam_paleta", (float)Paleta.tamanho_pal(Paleta.paleta_atual) - 1); // -1 pq o shader coloca +- pq a primeira or é bugada
	shader.setUniform("offset", (float)Paleta.offset_paleta);
	
	// reordenar_celulas(); //DEVE CHAMAR DEPOIS DE posiciona_celulas_tela POIS USA resolucao_aparente, calculada lá;

	// for (auto ItrCelula = ListaCelulasVivas.rbegin();ItrCelula != ListaCelulasVivas.rend();ItrCelula++)
	// {
	// Ccelula * Celula = *ItrCelula;
	// sf::Vector2f Posicao_tela = Celula->sprite.getPosition();
	// Csubcamada * sub = Celula->SubcamadaMae;
	// if(sub == nullptr) {
	// cout << "nullptr subcamada"; getchar();
	// }
	// float escalaCelulasTela = Celula->SubcamadaMae->Coordenadas.delta/DeltaAtual;
	// if (celula_esta_dentro(Celula,Propriedades.JanelaDesenho))
	// {
	// if (Celula->EstaProntaParaExibir)
	// {
	// JanelaSFML->draw(Celula->sprite,&shader);}
	// Celula->EstaDentroDaJanelaVisivel = true;
	// }
	// else
	// {
	// Celula->EstaDentroDaJanelaVisivel = false;
	// if (DEBUG)
	// {
	// Celula->sprite.setColor(COR_SPRITE_INVISIVEL);
	// if (Celula->resolucao_calculada<=RESOLUCAO_MAX) JanelaSFML->draw(Celula->sprite);
	// }
	// }
	// }

	for (auto ItrCamada = Camadas.begin(); ItrCamada != Camadas.end(); ItrCamada++)
	{
		Csubcamada* Subcamada = (*ItrCamada).second;
		for (int i = 0; i < COLUNAS_DE_SPRITES; i++)
		{
			for (int j = 0; j < LINHAS_DE_SPRITES; j++)
			{
				Ccelula* Celula = *Subcamada->PtrCelulas[i][j];
				if (Celula != nullptr) {
					// sf::Vector2f Posicao_tela = Celula->sprite.getPosition();
					Csubcamada* sub = Celula->SubcamadaMae;
					if (sub == nullptr) {
						cout << "nullptr subcamada"; getchar();
					}
					// float escalaCelulasTela = Celula->SubcamadaMae->Coordenadas.delta/DeltaAtual;
					if (celula_esta_dentro(Celula, Propriedades.JanelaDesenho))
					{
						if (Celula->EstaProntaParaExibir)
						{
							if (Celula->UsandoShader)
							JanelaSFML->draw(Celula->sprite, &shader);
							else JanelaSFML->draw(Celula->sprite);
						}
						Celula->EstaDentroDaJanelaVisivel = true;
					}
					else
					{
						Celula->EstaDentroDaJanelaVisivel = false;
						if (Propriedades.DebugCelulas)
						{
							Celula->sprite.setColor(COR_SPRITE_INVISIVEL);
							JanelaSFML->draw(Celula->sprite);
						}
					}
				}
			}
		}
	}



	if ((Propriedades.DebugCelulas))
	{
		CCoordenadas2DTela pos_tela_pivoAtual = CoordenadasPlanoAtualParaTela(pivoAtual);
		DebugObjects.Circulo_pivoTela.setPosition(pos_tela_pivoAtual.x, pos_tela_pivoAtual.y);
		DebugObjects.RtgPivoTela.setPosition(pivoTela.x, pivoTela.y);

		JanelaSFML->draw(DebugObjects.RtgAlocar); //DESENHA O RETANGULO VERMELHO DO DESENVOLVEDOR
		JanelaSFML->draw(DebugObjects.RtgDesalocar);
		JanelaSFML->draw(DebugObjects.RtgDesenhar);
		JanelaSFML->draw(DebugObjects.RtgPivoTela);
		JanelaSFML->draw(DebugObjects.Circulo_pivoTela);

		// gui.draw();
	}


	DebugObjects.textoDebug.setString(DebugObjects.ssdebug.str());

	if (Propriedades.DebugCamadas) JanelaSFML->draw(DebugObjects.textoDebug);
	JanelaSFML->display();
	DebugObjects.ssdebug.str(""); // limpa stringstream
}

void Cjanela::relatorioCamadas() {
	DebugObjects.ssdebug << "Camadas:\n";

	for (auto ItrCamada = Camadas.begin(); ItrCamada != Camadas.end(); ItrCamada++) {
		//Csubcamada * Subcamada =
		DebugObjects.ssdebug << (*ItrCamada).first << "\tTarefasFront: ";
		DebugObjects.ssdebug << (*ItrCamada).second->TarefasFrontPresentes.size() << endl;
	}
	DebugObjects.ssdebug << endl << "TarefasBack: " << Programa->TarefasBackPresentes.size() << endl;
	DebugObjects.ssdebug << endl << "Shader: " << (Propriedades.UsarShader?"ON":"OFF")<<endl;
}

void Cjanela::ApagarCelulasInuteis() {

	for (auto ItrCelula = ListaCelulasVivas.begin(); ItrCelula != ListaCelulasVivas.end(); ItrCelula++)
	{
		Ccelula* Celula = *ItrCelula;
		if (Celula == nullptr)
		{
			cout << "CELULA NULA AO DELETAR " << getchar();
		}
		if (Celula->marcadaParaDestruicao)
		{
			//  if (!(*ItrCelula)->esperandoTarefa)
			{
				// pode estar esperando tarefa back ou tarefa front ser convertida em back
				delete Celula;
				ItrCelula = ListaCelulasVivas.erase(ItrCelula);
				if (ItrCelula == ListaCelulasVivas.end()) break; // breaks for

			}
			//                 *Subcamada_atual->PtrCelulas[i][j] = nullptr;
		}
	}
}

void Cjanela::MarcarCelulasInuteis() {
	if (Camadas.empty())
	{
		cout << "Zero camadas  Cjanela::ApagarCelulasInuteis()";
	}

	for (auto CAMADA_ATUAL = Camadas.begin(); CAMADA_ATUAL != (Camadas.end()); CAMADA_ATUAL++) {
		(*CAMADA_ATUAL).second->MarcarCelulasInuteis();
	}
}

void Cjanela::reordenar_celulas()
{
	// funão desativada, mas caso reativr usar iteradores na ListaCelulasAtivas
}

Cjanela::~Cjanela() {
	
}
